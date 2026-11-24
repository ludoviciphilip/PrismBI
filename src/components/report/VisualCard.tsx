import React, { useMemo, useState, useRef, useCallback, useEffect } from 'react';
import ReactECharts from 'echarts-for-react';
import {
  Pin,
  Download,
  Trash2,
  Maximize2,
  Minimize2,
  Table as TableIcon,
  BarChart3,
  Filter,
  Layers,
  GripVertical,
  Target,
  ArrowLeft,
  ArrowRight,
  Calculator,
  ChevronRight,
  ChevronDown,
} from 'lucide-react';
import { Visual, Filter as BiFilter } from '../../types/bi';
import { useBiStore } from '../../store/useBiStore';
import { globalDataEngine, formatValue, formatCurrency, formatNumber, formatPercent, getCurrencySymbol } from '../../engine/dataEngine';
import { isTauriRuntime, queryVisualNative, NativeVisualQueryResult } from '../../bridge/tauriBridge';
import { ErrorBoundary } from '../shared/ErrorBoundary';
import { VisualCalculationModal } from '../modals/VisualCalculationModal';
import { useTranslation } from '../../i18n';
import { getEffectiveCanvasSize } from '../../engine/shared/canvasPresets';

interface VisualCardProps {
  visual: Visual;
  isSelected: boolean;
  onSelect: (id: string) => void;
  isMobile?: boolean;
}

const nativeQueryCache = new Map<string, NativeVisualQueryResult>();

function formatCompactDisplay(val: number, isCurr: boolean, currSymbol: string): string {
  const abs = Math.abs(val);
  const sign = val < 0 ? '-' : '';
  let scaled = abs;
  let suffix = '';
  if (abs >= 1_000_000_000) {
    scaled = abs / 1_000_000_000;
    suffix = ' Bi';
  } else if (abs >= 1_000_000) {
    scaled = abs / 1_000_000;
    suffix = ' Mi';
  } else if (abs >= 1_000) {
    scaled = abs / 1_000;
    suffix = ' Mil';
  } else {
    return isCurr ? formatCurrency(val) : formatNumber(val);
  }
  const numStr = scaled.toLocaleString('pt-BR', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  return isCurr ? `${sign}${currSymbol} ${numStr}${suffix}` : `${sign}${numStr}${suffix}`;
}

export const VisualCard: React.FC<VisualCardProps> = React.memo(({ visual, isSelected, onSelect, isMobile }) => {
  const cardRef = useRef<HTMLDivElement>(null);
  const filters = useBiStore(s => s.filters);
  const tables = useBiStore(s => s.tables);
  const measures = useBiStore(s => s.measures);
  const setCrossFilter = useBiStore(s => s.setCrossFilter);
  const updateVisual = useBiStore(s => s.updateVisual);
  const deleteVisual = useBiStore(s => s.deleteVisual);
  const pinVisualToDashboard = useBiStore(s => s.pinVisualToDashboard);
  const activePageId = useBiStore(s => s.activePageId);
  const executeDrillthrough = useBiStore(s => s.executeDrillthrough);
  const canvasZoom = useBiStore(s => s.canvasZoom);
  const activePalette = useBiStore(s => s.activePalette);
  const gridSize = useBiStore(s => s.gridSize);
  const snapToGrid = useBiStore(s => s.snapToGrid);
  const isDarkMode = useBiStore(s => s.isDarkMode);

  const { t, language, currency } = useTranslation();
  const currSymbol = getCurrencySymbol(currency);

  const [isFocused, setIsFocused] = useState(false);
  const [showRawTable, setShowRawTable] = useState(false);
  const [isDragging, setIsDragging] = useState(false);
  const [isVisualCalcModalOpen, setIsVisualCalcModalOpen] = useState(false);
  const [expandedMatrixRows, setExpandedMatrixRows] = useState<Record<string, boolean>>({});

  // Currency Field Detection
  const isCurrencyField = (fieldName: string): boolean => {
    if (!fieldName) return false;
    const lower = fieldName.toLowerCase();
    if (
      lower.includes('valor') ||
      lower.includes('preco') ||
      lower.includes('preço') ||
      lower.includes('receita') ||
      lower.includes('faturamento') ||
      lower.includes('custo') ||
      lower.includes('lucro') ||
      lower.includes('ticket') ||
      lower.includes('salario') ||
      lower.includes('pagamento') ||
      lower.includes('price') ||
      lower.includes('revenue') ||
      lower.includes('cost') ||
      lower.includes('profit') ||
      lower.includes('salary') ||
      lower.includes('payment') ||
      lower.includes('precio') ||
      lower.includes('ingreso') ||
      lower.includes('sueldo') ||
      (lower.includes('total') && !lower.includes('qtd') && !lower.includes('quantidade') && !lower.includes('count'))
    ) {
      return true;
    }
    const currentTableObj = tables.find(t => t.id === visual.dataConfig.tableId || t.name === visual.dataConfig.tableId);
    const colObj = currentTableObj?.columns.find(c => c.name.toLowerCase() === lower);
    if (colObj?.dataType === 'currency') return true;
    const measureObj = measures.find(m => m.name.toLowerCase() === lower);
    if (measureObj?.format === 'currency') return true;
    return false;
  };

  // Mouse Drag to move visual freely within canvas (DOM-driven at 60fps, commits to store on mouseup)
  const handleMouseDownDrag = (e: React.MouseEvent) => {
    if (isFocused || isMobile) return;
    // Don't drag if clicking buttons, inputs, selects, or tables
    const targetEl = e.target as HTMLElement;
    if (targetEl.closest('button') || targetEl.closest('input') || targetEl.closest('select') || targetEl.closest('table')) {
      return;
    }
    e.preventDefault();
    e.stopPropagation();
    onSelect(visual.id);
    setIsDragging(true);
    document.body.style.cursor = 'grabbing';
    document.body.style.userSelect = 'none';

    const startX = e.clientX;
    const startY = e.clientY;
    const origX = visual.x;
    const origY = visual.y;
    let lastX = origX;
    let lastY = origY;
    let rafId: number | null = null;
    const zoomScale = Math.max(0.2, (canvasZoom || 100) / 100);

    const activePage = useBiStore.getState().pages.find(p => p.id === activePageId);
    const { width: canvasWidth, height: canvasHeight } = getEffectiveCanvasSize(activePage);
    const step = snapToGrid ? (gridSize || 10) : 1;

    const onMouseMove = (moveEvent: MouseEvent) => {
      const deltaX = (moveEvent.clientX - startX) / zoomScale;
      const deltaY = (moveEvent.clientY - startY) / zoomScale;
      const rawX = origX + deltaX;
      const rawY = origY + deltaY;
      lastX = Math.max(0, Math.min(canvasWidth - visual.w, Math.round(rawX / step) * step));
      lastY = Math.max(0, Math.min(canvasHeight - visual.h, Math.round(rawY / step) * step));
      if (rafId !== null) cancelAnimationFrame(rafId);
      rafId = requestAnimationFrame(() => {
        if (cardRef.current) {
          cardRef.current.style.transform = `translate3d(${lastX - origX}px, ${lastY - origY}px, 0)`;
        }
        rafId = null;
      });
    };

    const onMouseUp = () => {
      if (rafId !== null) cancelAnimationFrame(rafId);
      if (cardRef.current) {
        cardRef.current.style.transform = '';
      }
      setIsDragging(false);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
      window.removeEventListener('mousemove', onMouseMove);
      window.removeEventListener('mouseup', onMouseUp);
      if (lastX !== origX || lastY !== origY) {
        updateVisual(visual.id, { x: lastX, y: lastY });
      }
    };

    window.addEventListener('mousemove', onMouseMove);
    window.addEventListener('mouseup', onMouseUp);
  };

  // Mouse drag to resize visual (DOM-driven at 60fps, commits to store on mouseup)
  const handleMouseDownResize = (e: React.MouseEvent) => {
    if (isMobile) return;
    e.preventDefault();
    e.stopPropagation();

    const activePage = useBiStore.getState().pages.find(p => p.id === activePageId);
    const { width: canvasWidth, height: canvasHeight } = getEffectiveCanvasSize(activePage);
    const zoomScale = Math.max(0.2, (canvasZoom || 100) / 100);
    const step = snapToGrid ? (gridSize || 10) : 10;

    const startX = e.clientX;
    const startY = e.clientY;
    const origW = visual.w;
    const origH = visual.h;
    let lastW = origW;
    let lastH = origH;
    let rafId: number | null = null;

    const onMouseMove = (moveEvent: MouseEvent) => {
      const deltaW = (moveEvent.clientX - startX) / zoomScale;
      const deltaH = (moveEvent.clientY - startY) / zoomScale;
      lastW = Math.max(160, Math.min(canvasWidth - visual.x, Math.round((origW + deltaW) / step) * step));
      lastH = Math.max(100, Math.min(canvasHeight - visual.y, Math.round((origH + deltaH) / step) * step));
      if (rafId !== null) cancelAnimationFrame(rafId);
      rafId = requestAnimationFrame(() => {
        if (cardRef.current) {
          cardRef.current.style.width = `${lastW}px`;
          cardRef.current.style.height = `${lastH}px`;
        }
        rafId = null;
      });
    };

    const onMouseUp = () => {
      if (rafId !== null) cancelAnimationFrame(rafId);
      window.removeEventListener('mousemove', onMouseMove);
      window.removeEventListener('mouseup', onMouseUp);
      if (lastW !== origW || lastH !== origH) {
        updateVisual(visual.id, { w: lastW, h: lastH });
      }
    };

    window.addEventListener('mousemove', onMouseMove);
    window.addEventListener('mouseup', onMouseUp);
  };

  // Compute visual data safely (never crash the page on a single visual DAX error)
  const { queryResult: tsQueryResult, queryError } = useMemo(() => {
    if (visual.type === 'action-button' || visual.type === 'textbox') {
      return {
        queryResult: { categories: [], series: [], rawRows: [] } as ReturnType<typeof globalDataEngine.queryVisualData>,
        queryError: null as string | null,
      };
    }
    try {
      return {
        queryResult: globalDataEngine.queryVisualData(visual, filters),
        queryError: null as string | null,
      };
    } catch (err: any) {
      return {
        queryResult: { categories: [], series: [], rawRows: [] } as ReturnType<typeof globalDataEngine.queryVisualData>,
        queryError: err?.message || String(err),
      };
    }
  }, [visual.id, visual.type, visual.dataConfig, visual.visualCalculations, filters, tables, measures]);

  const supportsNativeRustQuery =
    isTauriRuntime() &&
    visual.type !== 'action-button' &&
    visual.type !== 'textbox' &&
    visual.type !== 'table' &&
    visual.type !== 'scatter' &&
    !(visual.dataConfig.visualCalculations?.length || visual.visualCalculations?.length);

  const nativeCacheKey = useMemo(() => {
    if (!supportsNativeRustQuery) return '';
    return `${tables.length}:${measures.length}|${visual.id}|${visual.type}|${JSON.stringify(visual.dataConfig)}|${JSON.stringify(filters)}`;
  }, [supportsNativeRustQuery, tables.length, measures.length, visual.id, visual.type, visual.dataConfig, filters]);

  const [asyncNativeResult, setAsyncNativeResult] = useState<NativeVisualQueryResult | null>(() =>
    nativeCacheKey ? nativeQueryCache.get(nativeCacheKey) ?? null : null
  );

  useEffect(() => {
    if (!supportsNativeRustQuery || !nativeCacheKey) return;
    const cached = nativeQueryCache.get(nativeCacheKey);
    if (cached) {
      setAsyncNativeResult(cached);
      return;
    }

    let cancelled = false;
    const nativeFilters = filters
      .map(f => ({
        tableId: f.tableId,
        column: f.column,
        values: Array.isArray(f.value) ? f.value : f.value !== undefined && f.value !== '' ? [f.value] : [],
      }))
      .filter(f => f.values.length > 0);

    queryVisualNative({
      visualId: visual.id,
      visualType: visual.type,
      tableId: visual.dataConfig.tableId,
      xAxisField: visual.dataConfig.xAxisField,
      legendField: visual.dataConfig.legendField,
      yAxisFields: visual.dataConfig.yAxisFields || [],
      aggregation: visual.dataConfig.aggregation,
      filters: nativeFilters,
    })
      .then(res => {
        if (cancelled) return;
        if (nativeQueryCache.size >= 250) nativeQueryCache.clear();
        nativeQueryCache.set(nativeCacheKey, res);
        setAsyncNativeResult(res);
        useBiStore.getState().recordPerformanceMetric({
          visualId: visual.id,
          visualTitle: visual.title,
          queryTimeMs: res.queryTimeMs,
          renderTimeMs: tsQueryResult.queryTimeMs ?? 0,
          rowCount: res.categories.length,
        });
      })
      .catch(() => {
        // Fallback seamlessly to local TS QueryPlanner result
      });

    return () => {
      cancelled = true;
    };
  }, [supportsNativeRustQuery, nativeCacheKey]);

  const queryResult = useMemo(() => {
    if (supportsNativeRustQuery && asyncNativeResult) {
      return {
        ...tsQueryResult,
        categories: asyncNativeResult.categories,
        series: asyncNativeResult.series,
        summaryValue:
          asyncNativeResult.summaryValue !== null && asyncNativeResult.summaryValue !== undefined
            ? asyncNativeResult.summaryValue
            : tsQueryResult.summaryValue,
        queryTimeMs: asyncNativeResult.queryTimeMs,
      };
    }
    return tsQueryResult;
  }, [supportsNativeRustQuery, asyncNativeResult, tsQueryResult]);

  // State for interactive Drill-Through popover
  const [drillPop, setDrillPop] = useState<{
    field: string;
    value: any;
  } | null>(null);

  // Available destination pages for drillthrough (only resolved while popover is open)
  const targetPages = useMemo(() => {
    if (!drillPop) return [];
    return useBiStore.getState().pages.filter(p => p.id !== activePageId);
  }, [drillPop, activePageId]);

  // Handle cross-filtering & drill-through prompt when user clicks on a chart data point
  const onChartClick = useCallback((params: any) => {
    if (!params) return;
    const catName = params.name || params.data?.name;
    if (catName && catName !== 'Sem dados' && visual.dataConfig.xAxisField) {
      setCrossFilter(visual.dataConfig.tableId, visual.dataConfig.xAxisField, catName);
      setDrillPop({
        field: visual.dataConfig.xAxisField,
        value: catName,
      });
    }
  }, [visual.dataConfig.tableId, visual.dataConfig.xAxisField, setCrossFilter]);

  const onEvents = useMemo(() => ({
    click: onChartClick,
  }), [onChartClick]);

  // Build ECharts Options based on visual.type
  const echartsOption = useMemo(() => {
    const { categories, series } = queryResult;
    const colors = visual.formatConfig.colorPalette || activePalette || [
      '#f59e0b', '#3b82f6', '#10b981', '#8b5cf6', '#ec4899', '#06b6d4', '#64748b'
    ];

    const isYCurrency = isCurrencyField(visual.dataConfig.yAxisFields?.[0] || '');

    const axisColor = isDarkMode ? '#94a3b8' : '#64748b';
    const axisBorderColor = isDarkMode ? '#334155' : '#cbd5e1';
    const splitLineBorderColor = isDarkMode ? '#1e293b' : '#f1f5f9';
    const tooltipTitleColor = isDarkMode ? '#f8fafc' : '#1e293b';
    const tooltipValueColor = isDarkMode ? '#cbd5e1' : '#475569';
    const baseTooltip = {
      backgroundColor: isDarkMode ? '#0f172a' : '#ffffff',
      borderColor: isDarkMode ? '#334155' : '#e2e8f0',
      borderWidth: 1,
      textStyle: { color: tooltipTitleColor, fontSize: 12 },
      transitionDuration: 0,
      confine: true,
      extraCssText: 'pointer-events: none;',
    };

    const baseGrid = {
      top: 30,
      left: 50,
      right: 25,
      bottom: 35,
      containLabel: true,
    };

    switch (visual.type) {
      case 'column':
      case 'stacked-column':
        return {
          animation: false,
          color: colors,
          grid: baseGrid,
          tooltip: {
            ...baseTooltip,
            trigger: 'axis',
            axisPointer: { type: 'shadow' },
            formatter: (params: any[]) => {
              if (!params || !params[0]) return '';
              const cat = params[0].name;
              let html = `<div style="font-weight:600;color:${tooltipTitleColor};margin-bottom:4px;">${cat}</div>`;
              params.forEach(p => {
                const formattedVal = isYCurrency || isCurrencyField(p.seriesName) ? formatCurrency(p.value) : formatNumber(p.value);
                html += `<div class="flex justify-between items-center gap-4 text-xs" style="color:${tooltipValueColor}">
                  <span style="color:${p.color}">● ${p.seriesName}:</span>
                  <span class="font-bold" style="color:${tooltipTitleColor}">${formattedVal}</span>
                </div>`;
              });
              return html;
            },
          },
          xAxis: {
            type: 'category',
            data: categories,
            axisLabel: { rotate: categories.length > 5 ? 25 : 0, fontSize: 11, color: axisColor },
            axisLine: { lineStyle: { color: axisBorderColor } },
          },
          yAxis: {
            type: 'value',
            axisLabel: {
              fontSize: 10,
              color: axisColor,
              formatter: (v: number) => (isYCurrency ? (v >= 1000 ? `${currSymbol} ${(v / 1000).toFixed(0)}k` : `${currSymbol} ${v}`) : (v >= 1000 ? `${(v / 1000).toFixed(0)}k` : v)),
            },
            splitLine: { lineStyle: { type: 'dashed', color: splitLineBorderColor } },
          },
          series: series.map(s => ({
            name: s.name,
            type: 'bar',
            stack: visual.type === 'stacked-column' ? 'total' : undefined,
            data: s.data,
            itemStyle: { borderRadius: [4, 4, 0, 0] },
            label: {
              show: visual.formatConfig.showDataLabels ?? false,
              position: 'top',
              fontSize: 10,
              color: axisColor,
              textBorderWidth: 0,
              formatter: (p: any) => (isYCurrency ? formatCurrency(p.value) : p.value >= 1000 ? `${(p.value / 1000).toFixed(0)}k` : p.value),
            },
          })),
        };

      case 'bar':
      case 'stacked-bar':
        return {
          animation: false,
          color: colors,
          grid: baseGrid,
          tooltip: {
            ...baseTooltip,
            trigger: 'axis',
            axisPointer: { type: 'shadow' },
            formatter: (params: any[]) => {
              if (!params || !params[0]) return '';
              const cat = params[0].name;
              let html = `<div class="font-semibold mb-1" style="color:${tooltipTitleColor}">${cat}</div>`;
              params.forEach(p => {
                const formattedVal = isYCurrency || isCurrencyField(p.seriesName) ? formatCurrency(p.value) : formatNumber(p.value);
                html += `<div class="flex justify-between items-center gap-4 text-xs" style="color:${tooltipValueColor}">
                  <span style="color:${p.color}">● ${p.seriesName}:</span>
                  <span class="font-bold" style="color:${tooltipTitleColor}">${formattedVal}</span>
                </div>`;
              });
              return html;
            },
          },
          xAxis: {
            type: 'value',
            axisLabel: {
              fontSize: 10,
              color: axisColor,
              formatter: (v: number) => (isYCurrency ? (v >= 1000 ? `${currSymbol} ${(v / 1000).toFixed(0)}k` : `${currSymbol} ${v}`) : (v >= 1000 ? `${(v / 1000).toFixed(0)}k` : v)),
            },
            splitLine: { lineStyle: { type: 'dashed', color: splitLineBorderColor } },
          },
          yAxis: {
            type: 'category',
            data: categories,
            axisLabel: { fontSize: 11, color: axisColor },
            axisLine: { lineStyle: { color: axisBorderColor } },
          },
          series: series.map(s => ({
            name: s.name,
            type: 'bar',
            stack: visual.type === 'stacked-bar' ? 'total' : undefined,
            data: s.data,
            itemStyle: { borderRadius: [0, 4, 4, 0] },
            label: {
              show: visual.formatConfig.showDataLabels ?? false,
              position: 'right',
              fontSize: 10,
              color: axisColor,
            },
          })),
        };

      case 'line':
      case 'area':
        return {
          animation: false,
          color: colors,
          grid: baseGrid,
          tooltip: {
            ...baseTooltip,
            trigger: 'axis',
            formatter: (params: any[]) => {
              if (!params || !params[0]) return '';
              const cat = params[0].name;
              let html = `<div class="font-semibold mb-1" style="color:${tooltipTitleColor}">${cat}</div>`;
              params.forEach(p => {
                 const formattedVal = isYCurrency || isCurrencyField(p.seriesName) ? formatCurrency(p.value) : formatNumber(p.value);
                 html += `<div class="flex justify-between items-center gap-4 text-xs" style="color:${tooltipValueColor}">
                   <span style="color:${p.color}">● ${p.seriesName}:</span>
                   <span class="font-bold" style="color:${tooltipTitleColor}">${formattedVal}</span>
                 </div>`;
              });
              return html;
            },
          },
          xAxis: {
            type: 'category',
            data: categories,
            axisLabel: { fontSize: 11, color: axisColor },
            axisLine: { lineStyle: { color: axisBorderColor } },
          },
          yAxis: {
            type: 'value',
            axisLabel: {
              fontSize: 10,
              color: axisColor,
              formatter: (v: number) => (isYCurrency ? (v >= 1000 ? `${currSymbol} ${(v / 1000).toFixed(0)}k` : `${currSymbol} ${v}`) : (v >= 1000 ? `${(v / 1000).toFixed(0)}k` : v)),
            },
            splitLine: { lineStyle: { type: 'dashed', color: splitLineBorderColor } },
          },
          series: series.map(s => ({
            name: s.name,
            type: 'line',
            smooth: true,
            data: s.data,
            areaStyle: visual.type === 'area' ? { opacity: 0.25 } : undefined,
            label: {
              show: visual.formatConfig.showDataLabels ?? false,
              position: 'top',
              fontSize: 10,
              color: axisColor,
            },
          })),
        };

      case 'pie':
      case 'donut': {
        const rawPieData = categories.map((cat, i) => {
          const rawVal = series[0]?.data[i] ?? 0;
          const numVal = typeof rawVal === 'number' && !isNaN(rawVal) ? rawVal : Number(rawVal) || 0;
          return {
            name: cat || t.common.uncategorized,
            value: Math.abs(numVal), // Magnitude for geometric sector angles and percentage shares
            rawValue: numVal,        // Original signed value preserved for tooltips and labels
          };
        });

        // Filter out zero slices if there are non-zero slices, preventing clutter and overlapping labels
        const hasNonZero = rawPieData.some(p => p.value > 0);
        const nonZeroSlices = hasNonZero ? rawPieData.filter(p => p.value > 0) : rawPieData;
        const pieData = nonZeroSlices.length > 0
          ? nonZeroSlices
          : [{ name: t.common.noData, value: 0, rawValue: 0 }];

        // Find measure or column format
        const yFieldName = visual.dataConfig.yAxisFields?.[0] || '';
        const activeMeasure = measures.find(
          m => m.name.toLowerCase() === yFieldName.toLowerCase() || `Σ ${m.name}`.toLowerCase() === yFieldName.toLowerCase()
        );

        const formatVal = (val: number): string => {
          if (visual.formatConfig.displayUnits === 'auto') {
            const isCurr = activeMeasure?.format === 'currency' || isYCurrency || isCurrencyField(yFieldName);
            return formatCompactDisplay(val, isCurr, currSymbol);
          }
          if (activeMeasure) {
            if (activeMeasure.formatString) {
              return formatValue(val, activeMeasure.formatString);
            }
            if (activeMeasure.format === 'currency') return formatCurrency(val);
            if (activeMeasure.format === 'percentage') return formatPercent(val);
            if (activeMeasure.format === 'integer') return formatNumber(Math.round(val));
            if (activeMeasure.format === 'number') return formatNumber(val);
          }
          if (isYCurrency || isCurrencyField(yFieldName)) {
            return formatCurrency(val);
          }
          return formatNumber(val);
        };

        const donutTotal = hasNonZero
          ? nonZeroSlices.reduce((acc, item) => acc + item.rawValue, 0)
          : 0;

        return {
          animation: true,
          animationDuration: 0,
          animationDurationUpdate: 0,
          stateAnimation: { duration: 0 },
          color: colors,
          ...(visual.type === 'donut' && hasNonZero
            ? {
                title: {
                  text: formatVal(donutTotal),
                  left: 'center',
                  top: (visual.formatConfig.showLegend ?? true) ? '40%' : '46%',
                  textStyle: {
                    fontSize: 15,
                    fontWeight: 'bold',
                    color: tooltipTitleColor,
                  },
                },
              }
            : {}),
          tooltip: {
            ...baseTooltip,
            trigger: 'item',
            formatter: (p: any) => {
              if (p.name === t.common.noData) {
                return `<div style="font-weight:600;color:${axisColor};padding:2px 4px;">${t.visualCard.noDataToDisplay}</div>`;
              }
              const actualVal = p.data?.rawValue !== undefined ? p.data.rawValue : p.value;
              const valStr = formatVal(actualVal);
              const percentStr = typeof p.percent === 'number' ? `${p.percent.toFixed(1)}%` : '';
              return `<div style="font-weight:600;color:${tooltipTitleColor};margin-bottom:2px;">${p.name}</div>
                      <div style="font-size:12px;color:${tooltipValueColor};">${t.common.value}: <b style="color:${tooltipTitleColor};">${valStr}</b> ${percentStr ? `(${percentStr})` : ''}</div>`;
            },
          },
          legend: {
            show: visual.formatConfig.showLegend ?? true,
            type: 'scroll',
            bottom: 4,
            left: 'center',
            itemWidth: 10,
            itemHeight: 10,
            textStyle: { fontSize: 11, color: axisColor },
            pageIconSize: 10,
            pageTextStyle: { fontSize: 10, color: axisColor },
          },
          series: [
            {
              name: visual.title || (visual.type === 'donut' ? 'Gráfico de Rosca' : 'Gráfico de Pizza'),
              type: 'pie',
              radius: visual.type === 'donut' ? ['45%', '72%'] : '68%',
              center: ['50%', (visual.formatConfig.showLegend ?? true) ? '44%' : '50%'],
              avoidLabelOverlap: true,
              itemStyle: {
                borderRadius: visual.type === 'donut' ? 4 : 2,
                borderColor: isDarkMode ? '#1e293b' : '#ffffff',
                borderWidth: 2,
              },
              label: {
                show: visual.formatConfig.showDataLabels ?? true,
                formatter: (p: any) => {
                  if (p.name === t.common.noData || p.name === 'Sem dados') return '';
                  const actualVal = p.data?.rawValue !== undefined ? p.data.rawValue : p.value;
                  const percentStr = typeof p.percent === 'number' ? `${p.percent.toFixed(1)}%` : '';
                  const compactVal = formatCompactDisplay(actualVal, isYCurrency || isCurrencyField(yFieldName) || activeMeasure?.format === 'currency', currSymbol);
                  return percentStr ? `${p.name}: ${compactVal} (${percentStr})` : `${p.name}: ${compactVal}`;
                },
                fontSize: 10,
                color: axisColor,
              },
              labelLine: {
                show: visual.formatConfig.showDataLabels ?? true,
                length: 8,
                length2: 8,
              },
              data: pieData,
            },
          ],
        };
      }

      case 'treemap': {
        const treeData = categories.map((cat, i) => ({
          name: cat,
          value: series[0]?.data[i] || 0,
        }));
        return {
          animation: false,
          color: colors,
          tooltip: {
            ...baseTooltip,
            formatter: (p: any) => {
              const valStr = isYCurrency ? formatCurrency(p.value) : formatNumber(p.value);
              return `${p.name}: ${valStr}`;
            },
          },
          series: [
            {
              type: 'treemap',
              data: treeData,
              roam: false,
              label: {
                show: true,
                formatter: (p: any) => {
                  const valStr = isYCurrency ? formatCurrency(p.value) : p.value;
                  return `${p.name}\n${valStr}`;
                },
              },
              itemStyle: { gapWidth: 2, borderColor: isDarkMode ? '#1e293b' : '#fff' },
            },
          ],
        };
      }

      case 'waterfall': {
        const rawVals = series[0]?.data || [];
        const wfCategories = rawVals.length > 0 ? [...categories, 'Total'] : [...categories];
        const steps: [number, number, number, number, number][] = [];
        let acc = 0;
        rawVals.forEach((rawVal, idx) => {
          const delta = Number(rawVal) || 0;
          const start = acc;
          const end = acc + delta;
          steps.push([idx, start, end, delta, 0]);
          acc = end;
        });
        if (rawVals.length > 0) {
          steps.push([rawVals.length, 0, acc, acc, 1]);
        }

        const fmtWfVal = (v: number) =>
          visual.formatConfig.displayUnits === 'auto'
            ? formatCompactDisplay(v, isYCurrency, currSymbol)
            : isYCurrency
            ? formatCurrency(v)
            : formatNumber(v);

        return {
          animation: false,
          grid: {
            top: 32,
            left: 58,
            right: 24,
            bottom: 42,
            containLabel: true,
          },
          tooltip: {
            ...baseTooltip,
            trigger: 'axis',
            axisPointer: { type: 'shadow' },
            formatter: (params: any[]) => {
              const p = params?.[0];
              const row = p?.data as [number, number, number, number, number] | undefined;
              if (!row) return '';
              const [idx, , end, delta, isTotal] = row;
              const label = wfCategories[idx] || '';
              const deltaColor = isTotal === 1 ? '#3b82f6' : delta >= 0 ? '#10b981' : '#ef4444';
              const signPrefix = isTotal === 1 ? '' : delta > 0 ? '+' : '';
              return `<div style="font-weight:600;color:${tooltipTitleColor};margin-bottom:4px;">${label}</div>
                <div style="font-size:12px;color:${tooltipValueColor};">${isTotal === 1 ? 'Total' : 'Variação'}: <b style="color:${deltaColor};">${signPrefix}${fmtWfVal(delta)}</b></div>
                ${isTotal === 0 ? `<div style="font-size:11px;color:${tooltipValueColor};margin-top:2px;">Acumulado: <b style="color:${tooltipTitleColor};">${fmtWfVal(end)}</b></div>` : ''}`;
            },
          },
          xAxis: {
            type: 'category',
            data: wfCategories,
            axisLabel: { rotate: wfCategories.length > 6 ? 25 : 0, fontSize: 10, color: axisColor },
            axisLine: { lineStyle: { color: axisBorderColor } },
          },
          yAxis: {
            type: 'value',
            axisLabel: {
              fontSize: 10,
              color: axisColor,
              formatter: (v: number) => formatCompactDisplay(v, isYCurrency, currSymbol),
            },
            splitLine: { lineStyle: { type: 'dashed', color: splitLineBorderColor } },
          },
          series: [
            {
              name: visual.title || 'Cascata',
              type: 'custom',
              encode: { x: 0, y: [1, 2] },
              data: steps,
              renderItem: (_params: any, api: any) => {
                const idx = api.value(0) as number;
                const startVal = api.value(1) as number;
                const endVal = api.value(2) as number;
                const deltaVal = api.value(3) as number;
                const isTotal = api.value(4) === 1;

                const topPt = api.coord([idx, Math.max(startVal, endVal)]);
                const botPt = api.coord([idx, Math.min(startVal, endVal)]);
                const bandWidth = api.size([1, 0])[0] * 0.56;
                const barWidth = Math.max(8, Math.min(64, bandWidth));
                const rectHeight = Math.max(2, Math.abs(botPt[1] - topPt[1]));
                const xLeft = topPt[0] - barWidth / 2;
                const fill = isTotal ? '#3b82f6' : deltaVal >= 0 ? '#10b981' : '#ef4444';

                const children: any[] = [
                  {
                    type: 'rect',
                    shape: {
                      x: xLeft,
                      y: topPt[1],
                      width: barWidth,
                      height: rectHeight,
                      r: 3,
                    },
                    style: { fill },
                  },
                ];

                if (!isTotal && idx + 1 < wfCategories.length) {
                  const nextPt = api.coord([idx + 1, endVal]);
                  children.push({
                    type: 'line',
                    shape: {
                      x1: topPt[0] + barWidth / 2,
                      y1: nextPt[1],
                      x2: nextPt[0] - barWidth / 2,
                      y2: nextPt[1],
                    },
                    style: {
                      stroke: axisBorderColor,
                      lineWidth: 1,
                      lineDash: [3, 3],
                    },
                    silent: true,
                  });
                }

                if (visual.formatConfig.showDataLabels ?? true) {
                  const isDown = !isTotal && deltaVal < 0;
                  children.push({
                    type: 'text',
                    style: {
                      text: fmtWfVal(deltaVal),
                      x: topPt[0],
                      y: isDown ? botPt[1] + 5 : topPt[1] - 5,
                      fill: axisColor,
                      textAlign: 'center',
                      textVerticalAlign: isDown ? 'top' : 'bottom',
                      font: '600 10px sans-serif',
                    },
                    silent: true,
                  });
                }

                return {
                  type: 'group',
                  children,
                };
              },
            },
          ],
        };
      }

      case 'combo': {
        return {
          animation: false,
          color: colors,
          grid: baseGrid,
          tooltip: {
            ...baseTooltip,
            trigger: 'axis',
            axisPointer: { type: 'cross' },
          },
          legend: {
            show: visual.formatConfig.showLegend ?? true,
            bottom: 2,
            textStyle: { fontSize: 11, color: axisColor },
          },
          xAxis: {
            type: 'category',
            data: categories,
            axisLabel: { rotate: categories.length > 5 ? 25 : 0, fontSize: 11, color: axisColor },
            axisLine: { lineStyle: { color: axisBorderColor } },
          },
          yAxis: {
            type: 'value',
            axisLabel: {
              fontSize: 10,
              color: axisColor,
              formatter: (v: number) => (isYCurrency ? formatCurrency(v) : formatNumber(v)),
            },
            splitLine: { lineStyle: { type: 'dashed', color: splitLineBorderColor } },
          },
          series: series.map((s, idx) => ({
            name: s.name,
            type: idx === series.length - 1 && series.length > 1 ? 'line' : 'bar',
            smooth: true,
            data: s.data,
            itemStyle: { borderRadius: [4, 4, 0, 0] },
          })),
        };
      }

      case 'gauge': {
        const val = Number(queryResult.summaryValue ?? series[0]?.data[0] ?? 0) || 0;
        const target = visual.dataConfig.kpiTarget || Math.max(100, Math.abs(val) * 1.25);
        return {
          animation: false,
          tooltip: { ...baseTooltip, trigger: 'item' },
          series: [
            {
              type: 'gauge',
              min: 0,
              max: target,
              progress: { show: true, width: 12 },
              axisLine: { lineStyle: { width: 12 } },
              axisTick: { show: false },
              splitLine: { length: 8, lineStyle: { width: 2, color: axisColor } },
              axisLabel: { distance: 16, color: axisColor, fontSize: 10 },
              detail: {
                valueAnimation: false,
                fontSize: 16,
                color: tooltipTitleColor,
                formatter: (v: number) => (isYCurrency ? formatCurrency(v) : formatNumber(v)),
              },
              data: [{ value: val, name: visual.title }],
            },
          ],
        };
      }

      case 'funnel': {
        const funnelData = categories.map((cat, i) => ({
          name: cat,
          value: Math.abs(Number(series[0]?.data[i]) || 0),
          rawValue: Number(series[0]?.data[i]) || 0,
        }));
        const maxVal = Math.max(1, ...funnelData.map(d => d.value));
        const firstVal = funnelData[0]?.value ?? 0;
        const lastVal = funnelData[funnelData.length - 1]?.value ?? 0;
        const showConversion = funnelData.length >= 2 && firstVal > 0;
        const conversionPct = showConversion ? ((lastVal / firstVal) * 100).toFixed(1) : null;
        return {
          animation: false,
          color: colors,
          ...(showConversion
            ? {
                graphic: [
                  {
                    type: 'text',
                    left: 'center',
                    top: 6,
                    style: {
                      text: '100%',
                      fill: axisColor,
                      font: '600 10px sans-serif',
                    },
                  },
                  {
                    type: 'text',
                    left: 'center',
                    bottom: 4,
                    style: {
                      text: `${conversionPct}%`,
                      fill: axisColor,
                      font: '600 10px sans-serif',
                    },
                  },
                ],
              }
            : {}),
          tooltip: {
            ...baseTooltip,
            trigger: 'item',
            formatter: (p: any) => {
              const raw = p.data?.rawValue ?? p.value;
              const formatted = isYCurrency ? formatCurrency(raw) : formatNumber(raw);
              return `<div style="font-weight:600;color:${tooltipTitleColor}">${p.name}</div>
                      <div style="color:${tooltipValueColor}">${formatted}</div>`;
            },
          },
          series: [
            {
              name: visual.title,
              type: 'funnel',
              left: '8%',
              top: 26,
              bottom: 26,
              width: '84%',
              minSize: '22%',
              maxSize: '95%',
              min: 0,
              max: maxVal,
              sort: 'none',
              gap: 3,
              label: {
                show: true,
                position: 'inside',
                fontSize: 11,
                formatter: (p: any) => {
                  const raw = p.data?.rawValue ?? p.value;
                  const formatted = visual.formatConfig.displayUnits === 'auto'
                    ? formatCompactDisplay(raw, isYCurrency, currSymbol)
                    : (isYCurrency ? formatCurrency(raw) : formatNumber(raw));
                  return `${p.name}: ${formatted}`;
                },
              },
              data: funnelData,
            },
          ],
        };
      }

      case 'scatter': {
        const scatterData = queryResult.rawRows.map(r => [
          Number(r[visual.dataConfig.xAxisField || 'KmRodado']) || 0,
          Number(r[visual.dataConfig.yAxisFields?.[0] || 'CustoTotalViagem']) || 0,
        ]);
        return {
          animation: false,
          grid: baseGrid,
          tooltip: { ...baseTooltip, trigger: 'item' },
          xAxis: {
            type: 'value',
            name: visual.dataConfig.xAxisField,
            nameTextStyle: { color: axisColor },
            axisLabel: { color: axisColor },
            splitLine: { lineStyle: { type: 'dashed', color: splitLineBorderColor } },
          },
          yAxis: {
            type: 'value',
            name: visual.dataConfig.yAxisFields?.[0],
            nameTextStyle: { color: axisColor },
            axisLabel: { color: axisColor },
            splitLine: { lineStyle: { type: 'dashed', color: splitLineBorderColor } },
          },
          series: [
            {
              type: 'scatter',
              symbolSize: 12,
              itemStyle: { color: colors[0] },
              data: scatterData,
            },
          ],
        };
      }

      default:
        return {};
    }
  }, [visual.type, visual.title, visual.dataConfig, visual.formatConfig, queryResult, activePalette, isDarkMode]);

  const handleExportCSV = () => {
    if (!queryResult.rawRows.length) return;
    const keys = Object.keys(queryResult.rawRows[0]);
    const csvContent =
      'data:text/csv;charset=utf-8,' +
      [keys.join(';'), ...queryResult.rawRows.map(r => keys.map(k => `"${r[k] ?? ''}"`).join(';'))].join('\n');
    const encodedUri = encodeURI(csvContent);
    const link = document.createElement('a');
    link.setAttribute('href', encodedUri);
    link.setAttribute('download', `${visual.title.replace(/\s+/g, '_')}.csv`);
    document.body.appendChild(link);
    link.click();
    link.remove();
  };

  return (
    <div
      ref={cardRef}
      onClick={() => onSelect(visual.id)}
      onMouseDown={(e) => {
        if (isMobile) return;
        const target = e.target as HTMLElement;
        if (target.closest('button') || target.closest('input') || target.closest('select') || target.closest('td') || target.closest('th')) {
          return;
        }
        handleMouseDownDrag(e);
      }}
      style={{
        position: isMobile ? 'relative' : isFocused ? 'fixed' : 'absolute',
        top: isMobile ? undefined : isFocused ? 70 : visual.y,
        left: isMobile ? undefined : isFocused ? 70 : visual.x,
        width: isMobile ? '100%' : isFocused ? 'calc(100vw - 140px)' : visual.w,
        height: isMobile ? '100%' : isFocused ? 'calc(100vh - 140px)' : visual.h,
        zIndex: isFocused ? 9999 : isSelected ? 20 : visual.zIndex || 1,
        contain: isFocused || isDragging ? undefined : 'layout style',
        backgroundColor: visual.formatConfig.backgroundColor && visual.formatConfig.backgroundColor !== '#ffffff'
          ? visual.formatConfig.backgroundColor
          : (isDarkMode ? '#1e293b' : '#ffffff'),
        borderColor: isSelected
          ? '#eab308'
          : visual.formatConfig.borderColor || (isDarkMode ? '#334155' : '#e2e8f0'),
        borderRadius: visual.formatConfig.borderRadius ?? 8,
      }}
      className={`group flex flex-col border shadow-xs ${
        isMobile ? 'cursor-default' : 'cursor-grab active:cursor-grabbing'
      } ${isSelected ? 'ring-2 ring-amber-400' : ''} ${
        isDragging ? 'opacity-90 shadow-2xl ring-2 ring-amber-500 cursor-grabbing' : ''
      }`}
    >
      {/* Visual Header / Title Bar */}
      <div
        onMouseDown={isMobile ? undefined : handleMouseDownDrag}
        className={`flex items-center justify-between px-2.5 py-1.5 border-b border-slate-100 dark:border-slate-800 bg-slate-50/80 dark:bg-slate-800/80 select-none ${
          isMobile ? 'cursor-default' : 'cursor-move'
        } transition-colors ${isDragging ? 'bg-amber-100/70 dark:bg-amber-950/70' : 'hover:bg-slate-100/90 dark:hover:bg-slate-700/80'}`}
        title={isMobile ? visual.title : 'Clique e arraste para mover o visual pelo canvas'}
      >
        <div className="flex items-center gap-1.5 overflow-hidden">
          {!isMobile && <GripVertical className="w-3.5 h-3.5 text-slate-400 shrink-0" />}
          <span className="w-2 h-2 rounded-full bg-amber-500 shrink-0"></span>
          <span className="text-xs font-semibold text-slate-800 dark:text-slate-100 truncate" title={visual.title}>
            {visual.title}
          </span>
        </div>

        {/* Action Buttons */}
        <div className="flex items-center gap-1 opacity-80 group-hover:opacity-100">
          <button
            onClick={(e) => {
              e.stopPropagation();
              setShowRawTable(!showRawTable);
            }}
            title={showRawTable ? t.visualCard.viewChart : t.visualCard.viewTable}
            className="p-1 text-slate-500 dark:text-slate-400 hover:text-slate-800 dark:hover:text-slate-100 hover:bg-slate-200/60 dark:hover:bg-slate-700/60 rounded"
          >
            {showRawTable ? <BarChart3 className="w-3.5 h-3.5" /> : <TableIcon className="w-3.5 h-3.5" />}
          </button>
          <button
            onClick={(e) => {
              e.stopPropagation();
              pinVisualToDashboard(visual.id, activePageId, visual.title);
            }}
            title="Dashboard"
            className="p-1 text-slate-500 dark:text-slate-400 hover:text-amber-600 dark:hover:text-amber-400 hover:bg-slate-200/60 dark:hover:bg-slate-700/60 rounded"
          >
            <Pin className="w-3.5 h-3.5" />
          </button>
          <button
            onClick={(e) => {
              e.stopPropagation();
              handleExportCSV();
            }}
            title={t.visualCard.exportCsv}
            className="p-1 text-slate-500 dark:text-slate-400 hover:text-slate-800 dark:hover:text-slate-100 hover:bg-slate-200/60 dark:hover:bg-slate-700/60 rounded"
          >
            <Download className="w-3.5 h-3.5" />
          </button>
          <button
            onClick={(e) => {
              e.stopPropagation();
              const primaryField = visual.dataConfig.xAxisField || 'Categoria';
              const primaryVal = queryResult.categories[0] || 'Geral';
              setDrillPop(prev => (prev ? null : { field: primaryField, value: primaryVal }));
            }}
            title={t.visualCard.drillthrough}
            className={`p-1 rounded transition-colors ${
              drillPop ? 'text-amber-600 bg-amber-100 dark:bg-amber-950/60 font-bold' : 'text-slate-500 dark:text-slate-400 hover:text-amber-600 dark:hover:text-amber-400 hover:bg-slate-200/60 dark:hover:bg-slate-700/60'
            }`}
          >
            <Target className="w-3.5 h-3.5" />
          </button>
          <button
            onClick={(e) => {
              e.stopPropagation();
              setIsVisualCalcModalOpen(true);
            }}
            title={t.visualCard.openCalcModal}
            className={`p-1 rounded transition-colors ${
              ((visual.dataConfig?.visualCalculations?.length ?? 0) > 0 || (visual.visualCalculations?.length ?? 0) > 0)
                ? 'text-amber-600 bg-amber-100 dark:bg-amber-950/60 font-bold ring-1 ring-amber-300 dark:ring-amber-500'
                : 'text-slate-500 dark:text-slate-400 hover:text-amber-600 dark:hover:text-amber-400 hover:bg-slate-200/60 dark:hover:bg-slate-700/60'
            }`}
          >
            <Calculator className="w-3.5 h-3.5" />
          </button>
          <button
            onClick={(e) => {
              e.stopPropagation();
              setIsFocused(!isFocused);
            }}
            title={isFocused ? t.visualCard.restoreSize : t.visualCard.focusMode}
            className="p-1 text-slate-500 dark:text-slate-400 hover:text-slate-800 dark:hover:text-slate-100 hover:bg-slate-200/60 dark:hover:bg-slate-700/60 rounded"
          >
            {isFocused ? <Minimize2 className="w-3.5 h-3.5" /> : <Maximize2 className="w-3.5 h-3.5" />}
          </button>
          <button
            onClick={(e) => {
              e.stopPropagation();
              deleteVisual(visual.id);
            }}
            title={t.visualCard.deleteVisual}
            className="p-1 text-slate-400 hover:text-red-600 dark:hover:text-red-400 hover:bg-slate-200/60 dark:hover:bg-slate-700/60 rounded"
          >
            <Trash2 className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      {/* Visual Content Body */}
      <div className="flex-1 w-full h-full p-2 overflow-hidden flex flex-col justify-center">
        {queryError ? (
          <div className="p-3 rounded-lg bg-red-50 dark:bg-red-950/40 border border-red-200 dark:border-red-900 text-red-700 dark:text-red-300 text-[11px] overflow-auto">
            <div className="font-bold mb-1">Erro ao calcular visual</div>
            <div className="font-mono break-words">{queryError}</div>
          </div>
        ) : showRawTable ? (
          <div className="w-full h-full overflow-auto text-xs">
            <table className="min-w-full divide-y divide-slate-200 dark:divide-slate-700 text-left">
              <thead className="bg-slate-50 dark:bg-slate-800 sticky top-0">
                <tr>
                  {queryResult.rawRows[0] &&
                    Object.keys(queryResult.rawRows[0]).slice(0, 7).map(col => (
                      <th key={col} className="px-2 py-1 font-semibold text-slate-700 dark:text-slate-200">
                        {col}
                      </th>
                    ))}
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-100 dark:divide-slate-800 bg-white dark:bg-slate-900">
                {queryResult.rawRows.slice(0, 30).map((row, i) => (
                  <tr key={i} className="hover:bg-slate-50 dark:hover:bg-slate-800/60">
                    {Object.keys(row).slice(0, 7).map(col => (
                      <td key={col} className="px-2 py-1 text-slate-600 dark:text-slate-300 truncate max-w-[120px]">
                        {row[col] === null || row[col] === undefined
                          ? (visual.formatConfig?.showBlankAsZero ? (isCurrencyField(col) ? formatCurrency(0) : '0') : '')
                          : typeof row[col] === 'number' && isCurrencyField(col)
                          ? formatCurrency(row[col])
                          : String(row[col] ?? '')}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : visual.type === 'action-button' ? (
          <div className="w-full h-full flex items-center justify-center p-1">
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                const store = useBiStore.getState();
                if (visual.formatConfig.actionType === 'page-navigation' && visual.formatConfig.actionTargetPageId) {
                  store.setActivePageId(visual.formatConfig.actionTargetPageId);
                } else if (store.activeDrillthrough) {
                  store.clearDrillthrough(true);
                } else {
                  const curIdx = store.pages.findIndex(p => p.id === activePageId);
                  if (curIdx > 0) {
                    store.setActivePageId(store.pages[curIdx - 1].id);
                  }
                }
              }}
              className="w-full h-full flex items-center justify-center gap-1.5 rounded-lg bg-slate-100 hover:bg-amber-500 hover:text-slate-950 dark:bg-slate-800 dark:hover:bg-amber-500 text-slate-700 dark:text-slate-200 font-semibold text-xs border border-slate-300 dark:border-slate-700 transition-colors cursor-pointer"
              title={visual.title}
            >
              {visual.formatConfig.actionType === 'page-navigation' ? (
                <ArrowRight className="w-4 h-4 shrink-0" />
              ) : (
                <ArrowLeft className="w-4 h-4 shrink-0" />
              )}
              <span className="truncate">{visual.title || 'Voltar'}</span>
            </button>
          </div>
        ) : visual.type === 'textbox' ? (
          <div className="w-full h-full overflow-auto p-2 flex items-center text-xs text-slate-700 dark:text-slate-200 whitespace-pre-wrap leading-relaxed">
            {visual.formatConfig.textContent || visual.title}
          </div>
        ) : visual.type === 'card' || visual.type === 'kpi' ? (
          (() => {
            const isValBlank = queryResult.summaryValue === null || queryResult.summaryValue === undefined || isNaN(queryResult.summaryValue);
            const showZero = Boolean(visual.formatConfig?.showBlankAsZero);
            const val = isValBlank ? 0 : (queryResult.summaryValue ?? 0);
            const yField = visual.dataConfig.yAxisFields?.[0] || '';
            const targetMeasure = measures.find(m => m.name.toLowerCase() === yField.toLowerCase());
            const isPct = visual.title.includes('%') || yField.toLowerCase().includes('pct') || yField.toLowerCase().includes('percent') || targetMeasure?.format === 'percentage';
            const isCurr = isCurrencyField(yField) || targetMeasure?.format === 'currency' ||
              visual.title.toLowerCase().includes('r$') ||
              visual.title.toLowerCase().includes('$') ||
              visual.title.toLowerCase().includes('€') ||
              visual.title.toLowerCase().includes('faturamento') ||
              visual.title.toLowerCase().includes('lucro') ||
              visual.title.toLowerCase().includes('custo') ||
              visual.title.toLowerCase().includes('revenue') ||
              visual.title.toLowerCase().includes('profit') ||
              visual.title.toLowerCase().includes('cost') ||
              visual.title.toLowerCase().includes('precio');

            let displayText = '';
            if (!isValBlank || showZero) {
              if (visual.formatConfig.displayUnits === 'auto' && !isPct) {
                displayText = formatCompactDisplay(val, isCurr, currSymbol);
              } else if (targetMeasure?.formatString) {
                displayText = formatValue(val, targetMeasure.formatString, { showBlankAsZero: showZero, isLegacyColumn: false });
              } else if (isPct) {
                displayText = formatPercent(val);
              } else if (isCurr) {
                displayText = formatCurrency(val);
              } else {
                displayText = formatNumber(val);
              }
            }

            const signColorClass = visual.formatConfig.colorBySign && !isValBlank
              ? val > 0
                ? 'text-emerald-600 dark:text-emerald-400'
                : val < 0
                ? 'text-rose-600 dark:text-rose-400'
                : 'text-slate-800 dark:text-slate-100'
              : 'text-slate-800 dark:text-slate-100';

            return (
              <div className="flex flex-col items-center justify-center h-full text-center p-2">
                <span className={`text-3xl font-extrabold tracking-tight ${signColorClass}`}>
                  {displayText}
                </span>
                <span className="text-xs font-medium text-slate-500 dark:text-slate-400 mt-1">
                  {visual.dataConfig.tableId} • {visual.dataConfig.yAxisFields?.[0] || t.common.total} ({visual.dataConfig.aggregation || 'SUM'})
                </span>
              </div>
            );
          })()
        ) : visual.type === 'multi-card' ? (
          (() => {
            const cols = visual.formatConfig.cardColumns && visual.formatConfig.cardColumns > 0
              ? visual.formatConfig.cardColumns
              : Math.min(Math.max(1, queryResult.categories.length), 3);
            return (
              <div
                className="w-full h-full overflow-auto grid gap-2 p-2 items-center"
                style={{ gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))` }}
              >
                {queryResult.categories.map((label, idx) => {
                  const rawVal = Number(queryResult.series[0]?.data[idx] ?? 0) || 0;
                  const targetMeasure = measures.find(m => m.name.toLowerCase() === label.toLowerCase());
                  const isPct = targetMeasure?.format === 'percentage' || label.includes('%');
                  const isCurr = targetMeasure?.format === 'currency' || isCurrencyField(label);
                  let formatted: string;
                  if (visual.formatConfig.displayUnits === 'auto' && !isPct) {
                    formatted = formatCompactDisplay(rawVal, isCurr, currSymbol);
                  } else if (targetMeasure?.formatString) {
                    formatted = formatValue(rawVal, targetMeasure.formatString, {
                      showBlankAsZero: Boolean(visual.formatConfig?.showBlankAsZero),
                      isLegacyColumn: false,
                    });
                  } else if (isPct) {
                    formatted = formatPercent(rawVal);
                  } else if (isCurr) {
                    formatted = formatCurrency(rawVal);
                  } else {
                    formatted = formatNumber(rawVal);
                  }
                  const signColorClass = visual.formatConfig.colorBySign
                    ? rawVal > 0
                      ? 'text-emerald-600 dark:text-emerald-400'
                      : rawVal < 0
                      ? 'text-rose-600 dark:text-rose-400'
                      : 'text-slate-900 dark:text-slate-100'
                    : 'text-slate-900 dark:text-slate-100';
                  return (
                    <div
                      key={label}
                      className="p-2.5 rounded-lg border border-slate-200 dark:border-slate-700 bg-slate-50/70 dark:bg-slate-800/50 flex flex-col justify-center min-w-0"
                    >
                      <span className="text-[11px] font-semibold text-slate-500 dark:text-slate-400 truncate" title={label}>
                        {label}
                      </span>
                      <span className={`text-lg font-extrabold mt-0.5 truncate ${signColorClass}`}>
                        {formatted}
                      </span>
                    </div>
                  );
                })}
              </div>
            );
          })()
        ) : visual.type === 'slicer' ? (
          (() => {
            const rawSlicerCol = visual.dataConfig.xAxisField || 'Canal';
            const [hintTbl, hintCol] = rawSlicerCol.includes('.')
              ? rawSlicerCol.split('.', 2)
              : [visual.dataConfig.tableId, rawSlicerCol];
            const targetTableObj = tables.find(
              t => t.id === hintTbl || t.name === hintTbl || t.id === visual.dataConfig.tableId || t.name === visual.dataConfig.tableId
            );
            const slicerTableId = targetTableObj?.id || hintTbl;
            const slicerCol = hintCol;
            const activeSlicerFilter = filters.find(
              f =>
                (f.tableId === slicerTableId ||
                  f.tableId === visual.dataConfig.tableId ||
                  (targetTableObj && f.tableId === targetTableObj.name)) &&
                (f.column.toLowerCase() === slicerCol.toLowerCase() || f.column.toLowerCase() === rawSlicerCol.toLowerCase())
            );
            const activeVal = activeSlicerFilter ? String(activeSlicerFilter.value) : 'TODOS';
            const isTiles = visual.dataConfig.slicerConfig?.mode === 'tiles';

            return (
              <div className="flex flex-col h-full justify-center p-2 space-y-2 overflow-auto">
                <div className="flex items-center justify-between">
                  <label className="text-xs font-medium text-slate-600 dark:text-slate-300 flex items-center gap-1">
                    <Filter className="w-3 h-3 text-amber-500" /> {t.visualCard.selectField} {slicerCol}:
                  </label>
                  {activeVal !== 'TODOS' && (
                    <button
                      onClick={() => setCrossFilter(slicerTableId, slicerCol, '')}
                      className="text-[10px] text-amber-600 dark:text-amber-400 hover:text-amber-800 dark:hover:text-amber-300 font-semibold cursor-pointer"
                    >
                      {t.common.clear}
                    </button>
                  )}
                </div>
                {isTiles ? (
                  <div className="flex flex-wrap gap-1.5">
                    <button
                      type="button"
                      onClick={() => setCrossFilter(slicerTableId, slicerCol, '')}
                      className={`px-2 py-1 rounded text-[11px] font-semibold border transition-colors cursor-pointer ${
                        activeVal === 'TODOS'
                          ? 'bg-amber-500 text-slate-950 border-amber-500'
                          : 'bg-slate-50 dark:bg-slate-800 text-slate-700 dark:text-slate-300 border-slate-300 dark:border-slate-700 hover:border-amber-400'
                      }`}
                    >
                      {t.visualCard.allRecords}
                    </button>
                    {queryResult.categories.map(c => {
                      const selected = activeVal === c;
                      return (
                        <button
                          key={c}
                          type="button"
                          onClick={() => setCrossFilter(slicerTableId, slicerCol, selected ? '' : c)}
                          className={`px-2 py-1 rounded text-[11px] font-medium border transition-colors cursor-pointer ${
                            selected
                              ? 'bg-amber-500 text-slate-950 border-amber-500 font-bold'
                              : 'bg-slate-50 dark:bg-slate-800 text-slate-700 dark:text-slate-300 border-slate-300 dark:border-slate-700 hover:border-amber-400'
                          }`}
                        >
                          {c}
                        </button>
                      );
                    })}
                  </div>
                ) : (
                  <select
                    value={activeVal}
                    className="w-full text-xs bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-2 py-1.5 text-slate-800 dark:text-slate-100 focus:outline-none focus:ring-1 focus:ring-amber-500 font-medium"
                    onChange={(e) => {
                      const val = e.target.value;
                      setCrossFilter(slicerTableId, slicerCol, val === 'TODOS' ? '' : val);
                    }}
                  >
                    <option value="TODOS">{t.visualCard.allRecords}</option>
                    {queryResult.categories.map(c => (
                      <option key={c} value={c}>
                        {c}
                      </option>
                    ))}
                  </select>
                )}
              </div>
            );
          })()
        ) : visual.type === 'matrix' ? (
          (() => {
            const { categories, series } = queryResult;
            const colTotals = series.map(s => s.data.reduce((a, b) => a + (Number(b) || 0), 0));
            const rowTotals = categories.map((_, rIdx) =>
              series.reduce((sum, s) => sum + (Number(s.data[rIdx]) || 0), 0)
            );
            const grandTotal = rowTotals.reduce((a, b) => a + b, 0);
            const primaryY = visual.dataConfig.yAxisFields?.[0] || '';
            const useCurr = isCurrencyField(primaryY);
            const childHierarchyField = visual.dataConfig.drillFields?.[0];
            const fmtCell = (val: number, colName?: string) => {
              if (colName) {
                const m = measures.find(ms => ms.name.toLowerCase() === colName.toLowerCase());
                if (m?.formatString) return formatValue(val, m.formatString);
                if (m?.format === 'percentage' || colName.includes('%')) return formatPercent(val);
                if (m?.format === 'currency' || isCurrencyField(colName)) return formatCurrency(val);
              }
              return useCurr ? formatCurrency(val) : formatNumber(val);
            };

            const getChildRows = (parentCat: string) => {
              if (!childHierarchyField) return null;
              const rawParent = visual.dataConfig.xAxisField || '';
              const [pTbl, pCol] = rawParent.includes('.')
                ? rawParent.split('.', 2)
                : [visual.dataConfig.tableId, rawParent];
              const parentFilter: BiFilter = {
                id: `__matrix_expand_${visual.id}`,
                level: 'visual',
                tableId: pTbl,
                column: pCol,
                operator: 'equals',
                value: parentCat,
              };
              try {
                return globalDataEngine.queryVisualData(
                  {
                    ...visual,
                    id: `${visual.id}__child_${parentCat}`,
                    dataConfig: { ...visual.dataConfig, xAxisField: childHierarchyField },
                  },
                  [...filters, parentFilter]
                );
              } catch {
                return null;
              }
            };

            return (
              <div className="w-full h-full overflow-auto text-xs">
                <table className="min-w-full border-collapse text-left">
                  <thead className="bg-slate-100 dark:bg-slate-800 sticky top-0 z-10">
                    <tr>
                      <th className="px-2.5 py-1.5 font-bold text-slate-800 dark:text-slate-100 border-b border-r border-slate-200 dark:border-slate-700">
                        {visual.dataConfig.xAxisField || 'Linhas'}
                      </th>
                      {series.map(s => (
                        <th
                          key={s.name}
                          onClick={() => {
                            if (visual.dataConfig.legendField) {
                              setCrossFilter(visual.dataConfig.tableId, visual.dataConfig.legendField, s.name);
                            }
                          }}
                          className="px-2.5 py-1.5 font-semibold text-right text-slate-700 dark:text-slate-200 border-b border-slate-200 dark:border-slate-700 cursor-pointer hover:text-amber-600"
                        >
                          {s.name}
                        </th>
                      ))}
                      {series.length > 1 && (
                        <th className="px-2.5 py-1.5 font-bold text-right text-slate-900 dark:text-amber-300 border-b border-l border-slate-200 dark:border-slate-700 bg-slate-200/60 dark:bg-slate-800">
                          Total
                        </th>
                      )}
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-slate-100 dark:divide-slate-800 bg-white dark:bg-slate-900">
                    {categories.map((cat, rIdx) => {
                      const isExpanded = Boolean(childHierarchyField && expandedMatrixRows[cat]);
                      const childData = isExpanded ? getChildRows(cat) : null;
                      return (
                        <React.Fragment key={cat}>
                          <tr className="hover:bg-amber-50/50 dark:hover:bg-slate-800/60">
                            <td
                              onClick={() => {
                                if (visual.dataConfig.xAxisField) {
                                  setCrossFilter(visual.dataConfig.tableId, visual.dataConfig.xAxisField, cat);
                                }
                              }}
                              className="px-2.5 py-1 font-semibold text-slate-700 dark:text-slate-200 border-r border-slate-200 dark:border-slate-800 cursor-pointer hover:text-amber-600 whitespace-nowrap"
                            >
                              <div className="flex items-center gap-1">
                                {childHierarchyField && (
                                  <button
                                    type="button"
                                    onClick={(e) => {
                                      e.stopPropagation();
                                      setExpandedMatrixRows(prev => ({ ...prev, [cat]: !prev[cat] }));
                                    }}
                                    className="p-0.5 rounded hover:bg-slate-200 dark:hover:bg-slate-700 text-slate-500 cursor-pointer"
                                    title={isExpanded ? 'Recolher nível' : 'Expandir sub-nível'}
                                  >
                                    {isExpanded ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
                                  </button>
                                )}
                                <span>{cat}</span>
                              </div>
                            </td>
                            {series.map(s => (
                              <td
                                key={s.name}
                                className="px-2.5 py-1 text-right font-mono text-slate-600 dark:text-slate-300 whitespace-nowrap"
                              >
                                {fmtCell(Number(s.data[rIdx]) || 0, s.name)}
                              </td>
                            ))}
                            {series.length > 1 && (
                              <td className="px-2.5 py-1 text-right font-mono font-bold text-slate-900 dark:text-slate-100 border-l border-slate-200 dark:border-slate-800 bg-slate-50/70 dark:bg-slate-800/40 whitespace-nowrap">
                                {fmtCell(rowTotals[rIdx])}
                              </td>
                            )}
                          </tr>
                          {isExpanded && childData && childData.categories.map((childCat, cIdx) => {
                            const childRowTotal = childData.series.reduce((sum, cs) => sum + (Number(cs.data[cIdx]) || 0), 0);
                            return (
                              <tr key={`${cat}::${childCat}`} className="bg-slate-50/70 dark:bg-slate-800/30">
                                <td className="pl-7 pr-2.5 py-1 text-[11px] text-slate-600 dark:text-slate-300 border-r border-slate-200 dark:border-slate-800 whitespace-nowrap">
                                  {childCat}
                                </td>
                                {series.map((s, sIdx) => {
                                  const cs = childData.series.find(x => x.name === s.name) || childData.series[sIdx];
                                  return (
                                    <td
                                      key={s.name}
                                      className="px-2.5 py-1 text-right font-mono text-[11px] text-slate-500 dark:text-slate-400 whitespace-nowrap"
                                    >
                                      {fmtCell(Number(cs?.data[cIdx]) || 0, s.name)}
                                    </td>
                                  );
                                })}
                                {series.length > 1 && (
                                  <td className="px-2.5 py-1 text-right font-mono text-[11px] font-semibold text-slate-700 dark:text-slate-200 border-l border-slate-200 dark:border-slate-800 whitespace-nowrap">
                                    {fmtCell(childRowTotal)}
                                  </td>
                                )}
                              </tr>
                            );
                          })}
                        </React.Fragment>
                      );
                    })}
                  </tbody>
                  <tfoot className="bg-slate-100 dark:bg-slate-800/90 sticky bottom-0 font-bold border-t-2 border-slate-300 dark:border-slate-700">
                    <tr>
                      <td className="px-2.5 py-1.5 text-slate-900 dark:text-amber-300 border-r border-slate-200 dark:border-slate-700">
                        Total Geral
                      </td>
                      {colTotals.map((tot, cIdx) => (
                        <td
                          key={series[cIdx]?.name || cIdx}
                          className="px-2.5 py-1.5 text-right font-mono text-slate-900 dark:text-amber-300 whitespace-nowrap"
                        >
                          {fmtCell(tot, series[cIdx]?.name)}
                        </td>
                      ))}
                      {series.length > 1 && (
                        <td className="px-2.5 py-1.5 text-right font-mono text-slate-950 dark:text-amber-400 border-l border-slate-200 dark:border-slate-700 whitespace-nowrap">
                          {fmtCell(grandTotal)}
                        </td>
                      )}
                    </tr>
                  </tfoot>
                </table>
              </div>
            );
          })()
        ) : visual.type === 'table' ? (
          <div className="w-full h-full overflow-auto text-xs">
            <table className="min-w-full divide-y divide-slate-200 dark:divide-slate-700 text-left">
              <thead className="bg-slate-50 dark:bg-slate-800 sticky top-0">
                <tr>
                  {queryResult.rawRows[0] &&
                    Object.keys(queryResult.rawRows[0]).slice(0, 6).map(col => (
                      <th key={col} className="px-2 py-1 font-semibold text-slate-700 dark:text-slate-200 bg-slate-100 dark:bg-slate-800">
                        {col}
                      </th>
                    ))}
                  <th className="px-1.5 py-1 font-semibold text-slate-700 dark:text-slate-200 bg-slate-100 dark:bg-slate-800 w-12 text-center">Drill</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-100 dark:divide-slate-800 bg-white dark:bg-slate-900">
                {queryResult.rawRows.slice(0, 20).map((row, i) => {
                  const firstCol = Object.keys(row)[0] || 'Categoria';
                  const firstVal = row[firstCol];
                  return (
                    <tr key={i} className="hover:bg-amber-50/50 dark:hover:bg-slate-800/60 group/row">
                      {Object.keys(row).slice(0, 6).map(col => (
                        <td
                          key={col}
                          onClick={() => {
                            const val = row[col];
                            if (val !== undefined && val !== null) {
                              setCrossFilter(visual.dataConfig.tableId, col, val);
                              setDrillPop({ field: col, value: val });
                            }
                          }}
                          className="px-2 py-1 text-slate-600 dark:text-slate-300 truncate max-w-[120px] cursor-pointer hover:text-amber-800 dark:hover:text-amber-400 hover:font-semibold"
                          title={`Filtrar ou detalhar por ${col}: ${row[col]}`}
                        >
                          {typeof row[col] === 'number' && isCurrencyField(col)
                            ? formatCurrency(row[col])
                            : String(row[col] ?? '')}
                        </td>
                      ))}
                      <td className="px-1 py-0.5 text-center">
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            const primaryCol = visual.dataConfig.xAxisField || firstCol;
                            const primaryVal = row[primaryCol] !== undefined ? row[primaryCol] : firstVal;
                            setDrillPop({ field: primaryCol, value: primaryVal });
                          }}
                          className="p-1 text-slate-400 hover:text-amber-700 dark:hover:text-amber-400 hover:bg-amber-100 dark:hover:bg-slate-800 rounded transition-colors"
                          title="Fazer Drill-Through nesta linha"
                        >
                          <Target className="w-3 h-3" />
                        </button>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        ) : (
          <ErrorBoundary fallbackTitle="Falha na renderização deste gráfico">
            <ReactECharts
              key={`${visual.id}_${visual.type}`}
              option={echartsOption}
              notMerge={true}
              lazyUpdate={true}
              onEvents={onEvents}
              style={{ width: '100%', height: '100%' }}
              opts={{ renderer: 'canvas' }}
            />
          </ErrorBoundary>
        )}
      </div>

      {/* Floating Interactive Drill-Through Popover */}
      {drillPop && (
        <div
          onClick={(e) => e.stopPropagation()}
          onMouseDown={(e) => e.stopPropagation()}
          className="absolute z-50 bottom-2 right-2 min-w-[240px] max-w-[300px] bg-slate-900/95 backdrop-blur-md text-white rounded-xl shadow-2xl p-3 border border-slate-700 text-xs flex flex-col gap-2 animate-in fade-in zoom-in-95"
        >
          <div className="flex items-center justify-between border-b border-slate-700/80 pb-1.5">
            <div className="flex items-center gap-1.5 font-bold text-amber-400">
              <Target className="w-3.5 h-3.5" />
              <span>Detalhamento (Drill-Through)</span>
            </div>
            <button
              onClick={() => setDrillPop(null)}
              className="text-slate-400 hover:text-white px-1 font-bold text-xs cursor-pointer"
              title="Fechar"
            >
              ✕
            </button>
          </div>

          <div className="text-[11px] text-slate-300 leading-snug">
            Filtrar por <strong className="text-white">{drillPop.field}</strong> ={' '}
            <span className="text-amber-300 font-mono font-bold">"{String(drillPop.value)}"</span>
          </div>

          {targetPages.length > 0 ? (
            <div className="flex flex-col gap-1 max-h-40 overflow-y-auto pr-0.5 mt-0.5">
              <span className="text-[10px] uppercase tracking-wider text-slate-400 font-semibold">
                Navegar para página detalhada:
              </span>
              {targetPages.map(page => {
                const isRecommended =
                  page.drillthroughConfig?.isDrillthroughPage ||
                  page.name.toLowerCase().includes('detalh');
                return (
                  <button
                    key={page.id}
                    onClick={() => {
                      executeDrillthrough(page.id, drillPop.field, drillPop.value, visual.title);
                      setDrillPop(null);
                    }}
                    className={`flex items-center justify-between px-2.5 py-1.5 rounded-lg text-left text-xs font-semibold transition-all group cursor-pointer ${
                      isRecommended
                        ? 'bg-amber-500 hover:bg-amber-400 text-slate-950 shadow-xs ring-1 ring-amber-300'
                        : 'bg-slate-800 hover:bg-slate-700 text-slate-200 hover:text-white'
                    }`}
                  >
                    <div className="flex items-center gap-1.5 truncate">
                      {isRecommended && <Target className="w-3 h-3 shrink-0" />}
                      <span className="truncate">{page.name}</span>
                    </div>
                    <ArrowRight className="w-3 h-3 opacity-70 group-hover:opacity-100 group-hover:translate-x-0.5 transition-transform shrink-0" />
                  </button>
                );
              })}
            </div>
          ) : (
            <div className="text-[11px] text-slate-400 py-1">
              Nenhuma outra página para navegar. Crie uma nova página de detalhes na barra inferior.
            </div>
          )}
        </div>
      )}

      {/* Resize Handle Indicator */}
      {isSelected && !isFocused && !isMobile && (
        <>
          <div className="absolute -top-1 -left-1 w-2.5 h-2.5 bg-amber-500 rounded-full border border-white"></div>
          <div className="absolute -top-1 -right-1 w-2.5 h-2.5 bg-amber-500 rounded-full border border-white"></div>
          <div className="absolute -bottom-1 -left-1 w-2.5 h-2.5 bg-amber-500 rounded-full border border-white"></div>
          <div
            onMouseDown={handleMouseDownResize}
            className="absolute -bottom-1.5 -right-1.5 w-3 h-3 bg-amber-500 rounded-full border-2 border-white cursor-se-resize shadow-xs hover:scale-125 transition-transform"
            title="Arraste para redimensionar"
          ></div>
        </>
      )}

      {/* Visual Calculation Editor Modal */}
      {isVisualCalcModalOpen && (
        <VisualCalculationModal
          visual={visual}
          isOpen={true}
          onClose={() => setIsVisualCalcModalOpen(false)}
          onSave={(updatedCalcs) => {
            updateVisual(visual.id, {
              visualCalculations: updatedCalcs,
              dataConfig: {
                ...visual.dataConfig,
                visualCalculations: updatedCalcs,
              },
            });
          }}
        />
      )}
    </div>
  );
});
