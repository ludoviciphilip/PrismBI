import React, { useState, useEffect } from 'react';
import { createPortal } from 'react-dom';
import Papa from 'papaparse';
import * as XLSX from 'xlsx';
import {
  Upload,
  Database,
  Globe,
  FileSpreadsheet,
  CheckCircle2,
  AlertCircle,
  Server,
  Zap,
  CheckSquare,
  Square,
  Layers,
  ShieldCheck,
  Terminal,
  Info,
  ChevronDown,
  ChevronRight,
  Sparkles,
} from 'lucide-react';
import { useShallow } from 'zustand/react/shallow';
import { useBiStore } from '../../store/useBiStore';
import { mockDbAdapter, restApiAdapter } from '../../adapters/dataSourceAdapter';
import { postgresMockConnector, POSTGRES_TABLES, PostgresTableDef } from '../../adapters/postgresMockAdapter';
import { Table, Column, DataType, TableSourceConfig } from '../../types/bi';
import { DataImportService } from '../../services/dataImportService';
import {
  importCsvNative,
  importParquetNative,
  importJsonNative,
  importXlsxNative,
  isTauriRuntime,
  pickDataFile,
} from '../../bridge/tauriBridge';

interface SheetImportItem {
  sheetName: string;
  sourceSheetName?: string;
  tableName: string;
  columns: Column[];
  rows: Record<string, any>[];
  selected: boolean;
  targetMode: 'new' | 'replace';
  targetTableId: string;
}

export const DataSourceModal: React.FC = () => {
  const {
    dataSourceModalOpen,
    setDataSourceModalOpen,
    addTable,
    addTables,
    addRelationship,
    loadInitialSampleData,
    tables,
    replaceTargetTableId,
    setReplaceTargetTableId,
    replaceTableData,
  } = useBiStore(useShallow(s => ({
    dataSourceModalOpen: s.dataSourceModalOpen,
    setDataSourceModalOpen: s.setDataSourceModalOpen,
    addTable: s.addTable,
    addTables: s.addTables,
    addRelationship: s.addRelationship,
    loadInitialSampleData: s.loadInitialSampleData,
    tables: s.tables,
    replaceTargetTableId: s.replaceTargetTableId,
    setReplaceTargetTableId: s.setReplaceTargetTableId,
    replaceTableData: s.replaceTableData,
  })));
  const [activeTab, setActiveTab] = useState<'files' | 'postgres' | 'api' | 'db' | 'samples'>('files');
  const [nativeImporting, setNativeImporting] = useState(false);

  // File import state with multi-sheet support
  const [fileData, setFileData] = useState<{
    fileName: string;
    filePath?: string;
    sourceType: TableSourceConfig['type'];
    sheets: SheetImportItem[];
  } | null>(null);
  const [fileError, setFileError] = useState<string | null>(null);
  const [apiImporting, setApiImporting] = useState(false);

  useEffect(() => {
    if (dataSourceModalOpen && replaceTargetTableId) {
      setActiveTab('files');
      setFileData(null);
      setFileError(null);
    }
  }, [dataSourceModalOpen, replaceTargetTableId]);

  const closeModal = () => {
    setReplaceTargetTableId(null);
    setFileData(null);
    setFileError(null);
    setDataSourceModalOpen(false);
  };

  // PostgreSQL Mock state
  const [pgHost, setPgHost] = useState(postgresMockConnector.defaultHost);
  const [pgPort, setPgPort] = useState(postgresMockConnector.defaultPort);
  const [pgDatabase, setPgDatabase] = useState(postgresMockConnector.defaultDatabase);
  const [pgSchema, setPgSchema] = useState(postgresMockConnector.defaultSchema);
  const [pgUser, setPgUser] = useState(postgresMockConnector.defaultUser);
  const [pgTesting, setPgTesting] = useState(false);
  const [pgResult, setPgResult] = useState<any>(null);
  const [pgSelectedTables, setPgSelectedTables] = useState<string[]>([
    'pg_clientes',
    'pg_pedidos',
    'pg_itens_pedido',
    'pg_estoque_armazem',
  ]);
  const [pgExpandedTable, setPgExpandedTable] = useState<string | null>('pg_pedidos');
  const [pgQuery, setPgQuery] = useState('SELECT * FROM public.pg_pedidos LIMIT 10;');
  const [pgQueryResult, setPgQueryResult] = useState<any[] | null>(null);
  const [pgRunningQuery, setPgRunningQuery] = useState(false);
  const [pgImportSuccess, setPgImportSuccess] = useState(false);

  // Database connector state
  const [dbType, setDbType] = useState<'postgres' | 'mysql' | 'sqlserver' | 'oracle'>('sqlserver');
  const [dbHost, setDbHost] = useState('db.empresa.com.br');
  const [dbPort, setDbPort] = useState(1433);
  const [dbDatabase, setDbDatabase] = useState('ERP_PRODUCAO');
  const [dbUser, setDbUser] = useState('usr_powerbi');
  const [dbEnv, setDbEnv] = useState<'dev' | 'test' | 'prod'>('prod');
  const [useGateway, setUseGateway] = useState(true);
  const [gatewayUrl, setGatewayUrl] = useState('http://localhost:8080/gateway');
  const [dbTesting, setDbTesting] = useState(false);
  const [dbResult, setDbResult] = useState<any>(null);

  // API REST state
  const [apiUrl, setApiUrl] = useState('https://jsonplaceholder.typicode.com/posts');
  const [apiTesting, setApiTesting] = useState(false);
  const [apiResult, setApiResult] = useState<any>(null);

  if (!dataSourceModalOpen) return null;

  const replaceTargetTable = replaceTargetTableId
    ? tables.find(t => t.id === replaceTargetTableId || t.name === replaceTargetTableId) || null
    : null;

  const buildSheetItems = (
    rawSheets: { sheetName: string; sourceSheetName?: string; tableName: string; columns: Column[]; rows: Record<string, any>[] }[]
  ): SheetImportItem[] => {
    const preferredSheet = replaceTargetTable?.sourceConfig?.sheetName?.toLowerCase();
    let replaceMatchedIdx = -1;
    if (replaceTargetTable) {
      replaceMatchedIdx = rawSheets.findIndex(
        s =>
          (preferredSheet && (s.sourceSheetName || s.sheetName).toLowerCase() === preferredSheet) ||
          s.sheetName.toLowerCase() === replaceTargetTable.name.toLowerCase() ||
          s.tableName.toLowerCase() === replaceTargetTable.name.toLowerCase()
      );
      if (replaceMatchedIdx < 0) replaceMatchedIdx = 0;
    }

    return rawSheets.map((s, idx) => {
      if (replaceTargetTable) {
        const isTarget = idx === replaceMatchedIdx;
        return {
          ...s,
          selected: isTarget,
          targetMode: isTarget ? 'replace' : 'new',
          targetTableId: isTarget ? replaceTargetTable.id : '',
        };
      }
      return {
        ...s,
        selected: true,
        targetMode: 'new',
        targetTableId: '',
      };
    });
  };

  const handleNativeImportFile = async () => {
    setFileError(null);
    const path = await pickDataFile();
    if (!path) return;

    setNativeImporting(true);
    try {
      const lower = path.toLowerCase();
      let sourceType: TableSourceConfig['type'] = 'csv';
      let importedList;
      if (lower.endsWith('.xlsx') || lower.endsWith('.xls')) {
        sourceType = 'xlsx';
        importedList = await importXlsxNative(path);
      } else if (lower.endsWith('.parquet')) {
        sourceType = 'parquet';
        importedList = [await importParquetNative(path)];
      } else if (lower.endsWith('.json') || lower.endsWith('.ndjson')) {
        sourceType = 'json';
        importedList = [await importJsonNative(path)];
      } else {
        sourceType = 'csv';
        importedList = [await importCsvNative(path)];
      }

      const mapped = importedList.map(item => ({
        sheetName: item.sheetName || item.originalName,
        sourceSheetName: item.sheetName || item.originalName,
        tableName: item.tableName,
        columns: item.columns.map((c, idx) => ({
          id: `col_${idx}_${c.name}`,
          name: c.name,
          originalName: c.originalName,
          dataType: c.dataType || 'text',
        })),
        rows: item.rows,
      }));

      setFileData({
        fileName: path.split('/').pop() || path,
        filePath: path,
        sourceType,
        sheets: buildSheetItems(mapped),
      });
    } catch (err: any) {
      setFileError(err?.message || 'Erro ao importar arquivo via Rust + DuckDB.');
    } finally {
      setNativeImporting(false);
    }
  };

  // File Upload Handlers using DataImportService (CSV, Excel with all worksheets, JSON)
  const handleFileUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    setFileError(null);

    const ext = file.name.split('.').pop()?.toLowerCase();

    try {
      if (ext === 'csv' || ext === 'tsv' || ext === 'txt') {
        const result = await DataImportService.parseCsv(file);
        setFileData({
          fileName: file.name,
          sourceType: 'csv',
          sheets: buildSheetItems([
            {
              sheetName: result.tableName,
              tableName: result.tableName,
              columns: result.columns,
              rows: result.rows,
            },
          ]),
        });
      } else if (ext === 'xlsx' || ext === 'xls') {
        const result = await DataImportService.parseExcel(file);
        setFileData({
          fileName: file.name,
          sourceType: 'xlsx',
          sheets: buildSheetItems(
            result.sheets.map(s => ({
              ...s,
              sourceSheetName: s.sheetName,
            }))
          ),
        });
      } else if (ext === 'json') {
        const result = await DataImportService.parseJson(file);
        setFileData({
          fileName: file.name,
          sourceType: 'json',
          sheets: buildSheetItems([
            {
              sheetName: result.tableName,
              tableName: result.tableName,
              columns: result.columns,
              rows: result.rows,
            },
          ]),
        });
      } else {
        setFileError('Formato não suportado. Utilize .xlsx, .csv, .tsv, .parquet ou .json.');
      }
    } catch (err: any) {
      setFileError(err.message || 'Erro ao processar arquivo.');
    }
  };

  const handleToggleSheet = (sheetName: string) => {
    if (!fileData) return;
    setFileData({
      ...fileData,
      sheets: fileData.sheets.map(s =>
        s.sheetName === sheetName ? { ...s, selected: !s.selected } : s
      ),
    });
  };

  const handleSelectAllSheets = (selected: boolean) => {
    if (!fileData) return;
    setFileData({
      ...fileData,
      sheets: fileData.sheets.map(s => ({ ...s, selected })),
    });
  };

  const handleUpdateSheetMapping = (
    sheetName: string,
    patch: Partial<Pick<SheetImportItem, 'tableName' | 'targetMode' | 'targetTableId' | 'selected'>>
  ) => {
    if (!fileData) return;
    setFileData({
      ...fileData,
      sheets: fileData.sheets.map(s => (s.sheetName === sheetName ? { ...s, ...patch } : s)),
    });
  };

  const handleConfirmImportFile = () => {
    if (!fileData) return;
    const selectedSheets = fileData.sheets.filter(s => s.selected);
    if (selectedSheets.length === 0) {
      setFileError('Selecione ao menos uma planilha para carregar no modelo.');
      return;
    }

    const nowIso = new Date().toISOString();
    const newTables: Table[] = [];

    for (const s of selectedSheets) {
      const sheetConfig: TableSourceConfig = {
        type: fileData.sourceType,
        filePath: fileData.filePath,
        sheetName: fileData.sourceType === 'xlsx' ? (s.sourceSheetName || s.sheetName) : undefined,
        lastRefreshedAt: nowIso,
      };

      if (s.targetMode === 'replace' && s.targetTableId) {
        replaceTableData(s.targetTableId, s.columns, s.rows, sheetConfig);
      } else {
        const cleanName = (s.tableName || s.sheetName).trim().replace(/\s+/g, '_');
        newTables.push({
          id: cleanName,
          name: cleanName,
          originalName: s.sheetName,
          columns: s.columns,
          rows: s.rows,
          sourceConfig: sheetConfig,
        });
      }
    }

    if (newTables.length > 0) {
      addTables(newTables);
    }
    closeModal();
  };

  const handleImportRestApiData = async () => {
    setApiTesting(true);
    setApiImporting(true);
    setApiResult(null);
    try {
      const resp = await fetch(apiUrl);
      if (!resp.ok) {
        throw new Error(`Falha HTTP ${resp.status} ao buscar ${apiUrl}`);
      }
      const json = await resp.json();
      const rows: Record<string, any>[] = Array.isArray(json)
        ? json
        : Array.isArray((json as any)?.data)
        ? (json as any).data
        : (Object.values(json || {}).find(v => Array.isArray(v)) as Record<string, any>[]) || [];

      if (!Array.isArray(rows) || rows.length === 0) {
        throw new Error('O endpoint REST respondeu sem registros de array JSON.');
      }

      const keys = Array.from(new Set(rows.slice(0, 50).flatMap(r => Object.keys(r || {}))));
      const cols: Column[] = keys.map((key, idx) => {
        const sample = rows.find(r => r[key] !== null && r[key] !== undefined)?.[key];
        const dataType: DataType =
          typeof sample === 'number'
            ? 'number'
            : typeof sample === 'boolean'
            ? 'boolean'
            : typeof sample === 'string' && /^\d{4}-\d{2}-\d{2}/.test(sample)
            ? 'date'
            : 'text';
        return {
          id: `rest_col_${idx}_${key}`,
          name: key,
          originalName: key,
          dataType,
        };
      });

      const restConfig: TableSourceConfig = {
        type: 'rest',
        url: apiUrl,
        lastRefreshedAt: new Date().toISOString(),
      };

      if (replaceTargetTable) {
        replaceTableData(replaceTargetTable.id, cols, rows, restConfig);
      } else {
        const rawSlug = apiUrl.split('/').filter(Boolean).pop()?.replace(/[^a-zA-Z0-9_]/g, '_') || 'api_dados';
        const tblName = `rest_${rawSlug}`;
        addTable({
          id: tblName,
          name: tblName,
          originalName: tblName,
          columns: cols,
          rows,
          sourceConfig: restConfig,
        });
      }
      closeModal();
    } catch (err: any) {
      setApiResult({ success: false, message: err?.message || 'Erro ao importar JSON REST.' });
    } finally {
      setApiTesting(false);
      setApiImporting(false);
    }
  };

  const handleTestDatabase = async () => {
    setDbTesting(true);
    setDbResult(null);
    const res = await mockDbAdapter.testConnection({
      id: 'conn_' + Math.random(),
      name: 'Conexão Corporativa',
      type: dbType,
      host: dbHost,
      port: dbPort,
      database: dbDatabase,
      username: dbUser,
      environment: dbEnv,
      useGateway,
      gatewayUrl,
    });
    setDbResult(res);
    setDbTesting(false);
  };

  const handleImportMockDbTable = () => {
    const mockTbl: Table = {
      id: 'tb_faturamento_erp',
      name: 'tb_faturamento_erp',
      originalName: 'tb_faturamento_erp',
      columns: [
        { id: 'c1', name: 'ID_NotaFiscal', originalName: 'id_nf', dataType: 'number' },
        { id: 'c2', name: 'DataEmissao', originalName: 'data_emissao', dataType: 'date' },
        { id: 'c3', name: 'ValorTotal', originalName: 'valor_total_nf', dataType: 'currency' },
        { id: 'c4', name: 'StatusSEFAZ', originalName: 'status_sefaz', dataType: 'text' },
      ],
      rows: [
        { ID_NotaFiscal: 10091, DataEmissao: '2025-05-10', ValorTotal: 24500.0, StatusSEFAZ: 'Autorizada' },
        { ID_NotaFiscal: 10092, DataEmissao: '2025-05-11', ValorTotal: 18200.0, StatusSEFAZ: 'Autorizada' },
        { ID_NotaFiscal: 10093, DataEmissao: '2025-05-12', ValorTotal: 9400.0, StatusSEFAZ: 'Autorizada' },
      ],
    };
    addTable(mockTbl);
    setDataSourceModalOpen(false);
  };

  const handleTestPostgres = async () => {
    setPgTesting(true);
    setPgResult(null);
    const res = await postgresMockConnector.testConnection({
      host: pgHost,
      port: pgPort,
      database: pgDatabase,
      schema: pgSchema,
      user: pgUser,
    });
    setPgResult(res);
    setPgTesting(false);
  };

  const handleRunPostgresQuery = async () => {
    setPgRunningQuery(true);
    const res = await postgresMockConnector.executeQuery(pgQuery);
    setPgQueryResult(res.rows);
    setPgRunningQuery(false);
  };

  const handleTogglePgTable = (tableName: string) => {
    setPgSelectedTables(prev =>
      prev.includes(tableName) ? prev.filter(t => t !== tableName) : [...prev, tableName]
    );
  };

  const handleSelectAllPgTables = (select: boolean) => {
    setPgSelectedTables(select ? POSTGRES_TABLES.map(t => t.name) : []);
  };

  const handleImportPostgresTables = () => {
    const { tables: newTables, relationships: newRels } = postgresMockConnector.importTables(pgSelectedTables);
    if (newTables.length === 0) return;
    addTables(newTables);
    newRels.forEach(rel => addRelationship(rel));
    setPgImportSuccess(true);
    setTimeout(() => {
      setDataSourceModalOpen(false);
      setPgImportSuccess(false);
    }, 1200);
  };

  const handleTestApi = async () => {
    setApiTesting(true);
    setApiResult(null);
    const res = await restApiAdapter.testConnection({
      id: 'conn_api',
      name: 'API Externa',
      type: 'rest_api',
      url: apiUrl,
      environment: 'prod',
    });
    setApiResult(res);
    setApiTesting(false);
  };

  return createPortal(
    <div className="fixed inset-0 bg-black/60 z-[10000] flex items-center justify-center p-4">
      <div className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-2xl shadow-2xl max-w-3xl w-full p-6 text-xs flex flex-col max-h-[88vh] overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between pb-3 border-b border-slate-200 dark:border-slate-800">
          <div className="flex items-center gap-2">
            <div className="p-1.5 rounded-lg bg-amber-500 text-slate-950 font-bold">
              <Database className="w-4 h-4" />
            </div>
            <div>
              <h3 className="font-bold text-slate-900 dark:text-slate-100 text-sm">Obter Dados & Conectores</h3>
              <p className="text-[11px] text-slate-500 dark:text-slate-400">
                Importe ou substitua tabelas via Excel, CSV, Parquet, JSON, API REST ou banco de dados.
              </p>
            </div>
          </div>
          <button
            onClick={closeModal}
            className="text-slate-400 hover:text-slate-700 dark:hover:text-slate-200 text-sm font-bold p-1 cursor-pointer"
          >
            ✕
          </button>
        </div>

        {/* Tab Selector */}
        <div className="flex gap-2 py-3 border-b border-slate-100 dark:border-slate-800 flex-wrap">
          <button
            onClick={() => setActiveTab('postgres')}
            className={`px-3 py-1.5 rounded-lg font-bold transition-all flex items-center gap-1.5 cursor-pointer ${
              activeTab === 'postgres'
                ? 'bg-amber-500 text-slate-950 shadow-xs ring-1 ring-amber-400'
                : 'bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700'
            }`}
          >
            <span>🐘 PostgreSQL</span>
            <span className="text-[9px] bg-slate-900 dark:bg-slate-950 text-amber-300 font-extrabold px-1.5 py-0.5 rounded uppercase">
              Simulado
            </span>
          </button>
          <button
            onClick={() => setActiveTab('files')}
            className={`px-3 py-1.5 rounded-lg font-bold transition-all cursor-pointer ${
              activeTab === 'files' ? 'bg-amber-500 text-slate-950 shadow-xs' : 'bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700'
            }`}
          >
            Excel / CSV / Parquet / JSON
          </button>
          <button
            onClick={() => setActiveTab('api')}
            className={`px-3 py-1.5 rounded-lg font-bold transition-all cursor-pointer ${
              activeTab === 'api' ? 'bg-amber-500 text-slate-950 shadow-xs' : 'bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700'
            }`}
          >
            API REST / Web
          </button>
          <button
            onClick={() => setActiveTab('db')}
            className={`px-3 py-1.5 rounded-lg font-bold transition-all cursor-pointer ${
              activeTab === 'db' ? 'bg-amber-500 text-slate-950 shadow-xs' : 'bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700'
            }`}
          >
            Bancos SQL & Gateway
          </button>
          <button
            onClick={() => setActiveTab('samples')}
            className={`px-3 py-1.5 rounded-lg font-bold transition-all cursor-pointer ${
              activeTab === 'samples' ? 'bg-amber-500 text-slate-950 shadow-xs' : 'bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700'
            }`}
          >
            Amostras Prontas
          </button>
        </div>

        {/* Tab Body */}
        <div className="flex-1 overflow-y-auto py-3 space-y-4">
          {/* TAB POSTGRESQL (MOCK CONNECTOR) */}
          {activeTab === 'postgres' && (
            <div className="space-y-4">
              {/* Prominent Mock Notice & Indicator Banner */}
              <div className="p-4 bg-gradient-to-r from-amber-500/15 via-slate-100 to-amber-500/10 rounded-xl border-2 border-amber-500/50 space-y-2">
                <div className="flex items-center justify-between flex-wrap gap-2">
                  <div className="flex items-center gap-2">
                    <span className="text-2xl">🐘</span>
                    <div>
                      <div className="font-extrabold text-slate-900 text-sm flex items-center gap-2">
                        <span>PostgreSQL 16 Enterprise (Conector Simulado / Mock)</span>
                        <span className="px-2 py-0.5 rounded-full bg-amber-500 text-slate-950 font-black text-[9px] tracking-wider uppercase border border-amber-600 shadow-xs">
                          ⚡ Conector Mock Ativo
                        </span>
                      </div>
                      <p className="text-[11px] text-slate-600">
                        Servidor: <code className="bg-white px-1 py-0.5 rounded text-amber-900 font-mono font-semibold">{pgHost}:{pgPort}</code> • Banco: <code className="bg-white px-1 py-0.5 rounded text-amber-900 font-mono font-semibold">{pgDatabase}</code> • Esquema: <code className="bg-white px-1 py-0.5 rounded text-amber-900 font-mono font-semibold">{pgSchema}</code>
                      </p>
                    </div>
                  </div>

                  <span className="text-[10px] bg-slate-900 text-emerald-400 font-mono px-2 py-1 rounded-md flex items-center gap-1 font-bold">
                    <ShieldCheck className="w-3.5 h-3.5 text-emerald-400" />
                    SSL: TLSv1.3 Ativo
                  </span>
                </div>

                <div className="p-2.5 bg-amber-50/90 rounded-lg border border-amber-300 text-[11px] text-amber-950 flex items-start gap-2">
                  <Info className="w-4 h-4 text-amber-700 shrink-0 mt-0.5" />
                  <div className="leading-snug">
                    <strong>Ambiente Demonstrativo sem Gateway:</strong> Este conector simula em memória a experiência completa de conectar, consultar e modelar tabelas do PostgreSQL 16 com tipos nativos (<code className="bg-amber-100 px-1 py-0.2 rounded font-mono text-[10px]">SERIAL PRIMARY KEY</code>, <code className="bg-amber-100 px-1 py-0.2 rounded font-mono text-[10px]">NUMERIC(12,2)</code>, <code className="bg-amber-100 px-1 py-0.2 rounded font-mono text-[10px]">VARCHAR</code>, <code className="bg-amber-100 px-1 py-0.2 rounded font-mono text-[10px]">DATE</code>). Não requer credenciais reais nem gateways corporativos de rede.
                  </div>
                </div>
              </div>

              {/* Connection Parameters & Test Connection */}
              <div className="p-3 bg-slate-50 rounded-xl border border-slate-200 space-y-3">
                <div className="font-bold text-slate-800 text-xs flex items-center justify-between">
                  <span>Parâmetros de Conexão Simulada</span>
                  <span className="text-[10px] text-slate-500 font-normal">Porta padrão: 5432</span>
                </div>

                <div className="grid grid-cols-2 sm:grid-cols-4 gap-2">
                  <div>
                    <label className="text-[10px] font-semibold text-slate-600 block mb-0.5">Host / Endpoint</label>
                    <input
                      type="text"
                      value={pgHost}
                      onChange={(e) => setPgHost(e.target.value)}
                      className="w-full border rounded p-1.5 bg-white text-xs font-mono"
                    />
                  </div>
                  <div>
                    <label className="text-[10px] font-semibold text-slate-600 block mb-0.5">Porta</label>
                    <input
                      type="number"
                      value={pgPort}
                      onChange={(e) => setPgPort(Number(e.target.value))}
                      className="w-full border rounded p-1.5 bg-white text-xs font-mono"
                    />
                  </div>
                  <div>
                    <label className="text-[10px] font-semibold text-slate-600 block mb-0.5">Database</label>
                    <input
                      type="text"
                      value={pgDatabase}
                      onChange={(e) => setPgDatabase(e.target.value)}
                      className="w-full border rounded p-1.5 bg-white text-xs font-mono"
                    />
                  </div>
                  <div>
                    <label className="text-[10px] font-semibold text-slate-600 block mb-0.5">Usuário</label>
                    <input
                      type="text"
                      value={pgUser}
                      onChange={(e) => setPgUser(e.target.value)}
                      className="w-full border rounded p-1.5 bg-white text-xs font-mono"
                    />
                  </div>
                </div>

                <div className="flex items-center gap-3 pt-1">
                  <button
                    type="button"
                    onClick={handleTestPostgres}
                    disabled={pgTesting}
                    className="px-4 py-2 bg-slate-900 hover:bg-slate-800 text-white font-bold rounded-lg text-xs shadow-xs transition-colors flex items-center gap-1.5 cursor-pointer disabled:opacity-50"
                  >
                    <Zap className="w-3.5 h-3.5 text-amber-400" />
                    <span>{pgTesting ? 'Testando Conexão...' : 'Testar Conexão PostgreSQL (Simulado)'}</span>
                  </button>

                  {pgResult && (
                    <div className="text-[11px] font-medium text-emerald-700 flex items-center gap-1.5">
                      <CheckCircle2 className="w-4 h-4 text-emerald-600" />
                      <span>Conectado em {pgResult.latencyMs}ms • 4 tabelas localizadas</span>
                    </div>
                  )}
                </div>

                {pgResult && (
                  <div className="p-2.5 rounded-lg bg-emerald-50 border border-emerald-200 text-emerald-900 text-[11px] space-y-1">
                    <div className="font-bold flex items-center gap-1">
                      <span>✓ {pgResult.message}</span>
                    </div>
                    <div className="text-[10px] text-emerald-800/80 font-mono">
                      {pgResult.diagnostics.version}
                    </div>
                  </div>
                )}
              </div>

              {/* PostgreSQL Schema Catalog & Table Selection */}
              <div className="space-y-2">
                <div className="flex items-center justify-between pb-1">
                  <div>
                    <span className="font-bold text-slate-800 text-xs">
                      Tabelas do Esquema PostgreSQL ({POSTGRES_TABLES.length} disponíveis)
                    </span>
                    <p className="text-[10px] text-slate-500">
                      Selecione as tabelas relacionais para carregar no modelo do InsightHub
                    </p>
                  </div>
                  <div className="flex items-center gap-2 text-[10px]">
                    <button
                      type="button"
                      onClick={() => handleSelectAllPgTables(true)}
                      className="px-2 py-0.5 rounded bg-slate-100 hover:bg-slate-200 text-slate-700 font-semibold cursor-pointer"
                    >
                      Selecionar Todas
                    </button>
                    <button
                      type="button"
                      onClick={() => handleSelectAllPgTables(false)}
                      className="px-2 py-0.5 rounded bg-white border border-slate-200 hover:bg-slate-100 text-slate-600 font-semibold cursor-pointer"
                    >
                      Desmarcar Todas
                    </button>
                  </div>
                </div>

                <div className="space-y-2 max-h-64 overflow-y-auto pr-1">
                  {POSTGRES_TABLES.map(table => {
                    const isSelected = pgSelectedTables.includes(table.name);
                    const isExpanded = pgExpandedTable === table.name;
                    return (
                      <div
                        key={table.name}
                        className={`rounded-xl border transition-all ${
                          isSelected ? 'bg-amber-50/30 border-amber-300' : 'bg-slate-50 border-slate-200 opacity-70'
                        }`}
                      >
                        <div
                          onClick={() => handleTogglePgTable(table.name)}
                          className="p-3 flex items-center justify-between cursor-pointer"
                        >
                          <div className="flex items-center gap-2.5">
                            <input
                              type="checkbox"
                              checked={isSelected}
                              onChange={() => {}}
                              className="w-4 h-4 text-amber-600 rounded border-slate-300 focus:ring-amber-500"
                            />
                            <div>
                              <div className="font-bold text-slate-900 text-xs flex items-center gap-2">
                                <span className="font-mono text-amber-900">{table.schema}.{table.name}</span>
                                <span className="text-[10px] text-slate-400 font-normal">
                                  ({table.rows.length} registros • {table.columns.length} colunas)
                                </span>
                              </div>
                              <div className="text-[10px] text-slate-500 mt-0.5">
                                {table.description}
                              </div>
                            </div>
                          </div>

                          <div className="flex items-center gap-2">
                            <button
                              type="button"
                              onClick={(e) => {
                                e.stopPropagation();
                                setPgExpandedTable(isExpanded ? null : table.name);
                              }}
                              className="text-[10px] font-semibold text-slate-600 hover:text-slate-900 px-2 py-0.5 rounded bg-white border border-slate-200 flex items-center gap-1 cursor-pointer"
                            >
                              <span>{isExpanded ? 'Ocultar Colunas' : 'Inspecionar Tipos'}</span>
                              {isExpanded ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
                            </button>
                          </div>
                        </div>

                        {/* Expanded Column Schema Inspector */}
                        {isExpanded && (
                          <div className="p-3 pt-0 border-t border-slate-200/80 bg-white/60 rounded-b-xl overflow-x-auto">
                            <table className="min-w-full text-[10px] divide-y divide-slate-200">
                              <thead>
                                <tr className="text-left font-semibold text-slate-600">
                                  <th className="py-1 px-1.5">Coluna</th>
                                  <th className="py-1 px-1.5">Tipo PostgreSQL</th>
                                  <th className="py-1 px-1.5">Mapeamento BI</th>
                                  <th className="py-1 px-1.5">Descrição</th>
                                </tr>
                              </thead>
                              <tbody className="divide-y divide-slate-100 font-mono">
                                {table.columns.map(col => (
                                  <tr key={col.name} className="hover:bg-slate-50">
                                    <td className="py-1 px-1.5 font-bold text-slate-800">
                                      {col.name} {col.isPrimary && <span className="text-amber-600 font-sans text-[9px]">(PK)</span>}
                                      {col.isForeign && <span className="text-blue-600 font-sans text-[9px]">(FK)</span>}
                                    </td>
                                    <td className="py-1 px-1.5 text-purple-700">{col.pgType}</td>
                                    <td className="py-1 px-1.5 text-emerald-700 uppercase">{col.biType}</td>
                                    <td className="py-1 px-1.5 font-sans text-slate-500 text-[10px]">{col.description}</td>
                                  </tr>
                                ))}
                              </tbody>
                            </table>
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
              </div>

              {/* Interactive SQL Query Sandbox */}
              <div className="p-3 bg-slate-900 text-slate-100 rounded-xl space-y-2">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-1.5 text-amber-400 font-bold text-xs">
                    <Terminal className="w-3.5 h-3.5" />
                    <span>Console SQL PostgreSQL (Simulado)</span>
                  </div>
                  <div className="flex items-center gap-1 text-[10px]">
                    <span className="text-slate-400">Exemplos rápidos:</span>
                    <button
                      type="button"
                      onClick={() => setPgQuery('SELECT * FROM public.pg_pedidos WHERE valor_liquido > 30000;')}
                      className="px-1.5 py-0.5 rounded bg-slate-800 hover:bg-slate-700 text-amber-300 font-mono text-[9px] cursor-pointer"
                    >
                      pedidos &gt; 30k
                    </button>
                    <button
                      type="button"
                      onClick={() => setPgQuery('SELECT * FROM public.pg_clientes;')}
                      className="px-1.5 py-0.5 rounded bg-slate-800 hover:bg-slate-700 text-amber-300 font-mono text-[9px] cursor-pointer"
                    >
                      clientes
                    </button>
                    <button
                      type="button"
                      onClick={() => setPgQuery('SELECT * FROM public.pg_itens_pedido;')}
                      className="px-1.5 py-0.5 rounded bg-slate-800 hover:bg-slate-700 text-amber-300 font-mono text-[9px] cursor-pointer"
                    >
                      itens_pedido
                    </button>
                  </div>
                </div>

                <div className="flex gap-2">
                  <input
                    type="text"
                    value={pgQuery}
                    onChange={(e) => setPgQuery(e.target.value)}
                    className="flex-1 bg-slate-950 text-emerald-400 font-mono text-xs px-2.5 py-1.5 rounded-lg border border-slate-700 focus:outline-none focus:border-amber-400"
                  />
                  <button
                    type="button"
                    onClick={handleRunPostgresQuery}
                    disabled={pgRunningQuery}
                    className="px-3 py-1.5 bg-amber-500 hover:bg-amber-400 text-slate-950 font-bold rounded-lg text-xs transition-colors cursor-pointer shrink-0"
                  >
                    {pgRunningQuery ? 'Executando...' : 'Executar'}
                  </button>
                </div>

                {pgQueryResult && (
                  <div className="max-h-36 overflow-auto border border-slate-800 rounded-lg bg-slate-950 p-2 text-[10px]">
                    <table className="min-w-full font-mono divide-y divide-slate-800">
                      <thead>
                        <tr className="text-left text-slate-400">
                          {pgQueryResult[0] &&
                            Object.keys(pgQueryResult[0]).slice(0, 6).map(k => (
                              <th key={k} className="px-1.5 py-0.5">{k}</th>
                            ))}
                        </tr>
                      </thead>
                      <tbody className="divide-y divide-slate-900 text-slate-300">
                        {pgQueryResult.slice(0, 5).map((row, idx) => (
                          <tr key={idx}>
                            {Object.keys(row).slice(0, 6).map(k => (
                              <td key={k} className="px-1.5 py-0.5 truncate max-w-[120px]">
                                {String(row[k] ?? '')}
                              </td>
                            ))}
                          </tr>
                        ))}
                      </tbody>
                    </table>
                    <div className="text-[9px] text-slate-500 mt-1">
                      {pgQueryResult.length} linhas retornadas pelo mock PostgreSQL
                    </div>
                  </div>
                )}
              </div>

              {/* Import Action Bar */}
              <div className="flex items-center justify-between pt-2 border-t border-slate-200">
                <span className="text-[11px] text-slate-600 font-medium">
                  {pgSelectedTables.length} tabela{pgSelectedTables.length !== 1 ? 's' : ''} selecionada{pgSelectedTables.length !== 1 ? 's' : ''} para o modelo
                </span>

                <button
                  type="button"
                  onClick={handleImportPostgresTables}
                  disabled={pgSelectedTables.length === 0}
                  className="px-5 py-2.5 bg-amber-500 hover:bg-amber-400 text-slate-950 font-extrabold rounded-xl text-xs shadow-md transition-all flex items-center gap-2 cursor-pointer disabled:opacity-50 hover:scale-101"
                >
                  <Sparkles className="w-4 h-4 text-slate-950" />
                  <span>
                    {pgImportSuccess
                      ? '✓ Tabelas PostgreSQL Importadas com Sucesso!'
                      : `Importar ${pgSelectedTables.length} Tabelas Selecionadas no Modelo BI`}
                  </span>
                </button>
              </div>
            </div>
          )}
          {/* TAB 1: LOCAL FILES */}
          {activeTab === 'files' && (
            <div className="space-y-4">
              {replaceTargetTable && (
                <div className="p-3 rounded-xl bg-blue-50 dark:bg-blue-950/40 border border-blue-300 dark:border-blue-800 flex items-center justify-between gap-3 text-blue-950 dark:text-blue-200">
                  <div className="text-[11px] leading-snug">
                    <strong>Modo de Substituição de Tabela:</strong> você está substituindo os dados de{' '}
                    <code className="px-1.5 py-0.5 rounded bg-white dark:bg-slate-900 font-bold">{replaceTargetTable.name}</code>.{' '}
                    Escolha abaixo qualquer arquivo ou aba (ex.: aba &quot;Fato&quot;) — medidas, visuais, relacionamentos e filtros serão preservados.
                  </div>
                  <button
                    type="button"
                    onClick={() => setReplaceTargetTableId(null)}
                    className="px-2 py-1 rounded bg-white dark:bg-slate-800 border border-blue-200 dark:border-blue-700 text-[10px] font-semibold shrink-0 cursor-pointer"
                  >
                    Importar como nova
                  </button>
                </div>
              )}
              {isTauriRuntime() && (
                <div className="p-3.5 rounded-xl bg-amber-50 dark:bg-amber-950/30 border border-amber-300 dark:border-amber-800 flex items-center justify-between gap-3">
                  <div>
                    <div className="font-bold text-slate-900 dark:text-slate-100 text-xs flex items-center gap-1.5">
                      <Zap className="w-3.5 h-3.5 text-amber-500" />
                      <span>Importação Nativa (Rust + DuckDB)</span>
                    </div>
                    <p className="text-[11px] text-slate-600 dark:text-slate-400 mt-0.5">
                      Lê Excel (.xlsx/.xls), CSV, Parquet e JSON diretamente do disco no motor DuckDB nativo e vincula a fonte para futuras atualizações.
                    </p>
                  </div>
                  <button
                    type="button"
                    onClick={handleNativeImportFile}
                    disabled={nativeImporting}
                    className="px-4 py-2 rounded-lg bg-amber-500 hover:bg-amber-400 disabled:opacity-50 text-slate-950 font-bold text-xs shadow-xs shrink-0 cursor-pointer"
                  >
                    {nativeImporting ? 'Importando...' : 'Abrir Arquivo Nativo'}
                  </button>
                </div>
              )}
              <label className="border-2 border-dashed border-slate-300 hover:border-amber-400 rounded-xl p-8 flex flex-col items-center justify-center cursor-pointer transition-colors bg-slate-50">
                <Upload className="w-8 h-8 text-amber-500 mb-2" />
                <span className="font-bold text-slate-800 text-sm">
                  Clique ou arraste seu arquivo aqui
                </span>
                <span className="text-slate-400 text-[11px] mt-1">
                  Formatos suportados: Excel (.xlsx, .xls), CSV, TSV, Parquet e JSON
                </span>
                <input
                  type="file"
                  accept=".csv,.tsv,.txt,.xlsx,.xls,.json"
                  onChange={handleFileUpload}
                  className="hidden"
                />
              </label>

              {fileError && (
                <div className="p-3 rounded-lg bg-red-50 border border-red-200 text-red-700 font-medium">
                  {fileError}
                </div>
              )}

              {fileData && (
                <div className="p-4 rounded-xl border border-emerald-200 bg-emerald-50/50 space-y-3">
                  <div className="flex items-center justify-between pb-2 border-b border-emerald-200/60">
                    <div className="flex items-center gap-2">
                      <div className="p-1.5 rounded-lg bg-emerald-600 text-white font-bold">
                        <FileSpreadsheet className="w-4 h-4" />
                      </div>
                      <div>
                        <span className="font-bold text-emerald-950 text-sm">
                          {fileData.fileName}
                        </span>
                        <div className="text-[11px] text-emerald-700">
                          {fileData.sheets.length === 1
                            ? '1 planilha / tabela detectada'
                            : `${fileData.sheets.length} abas / planilhas detectadas`}
                        </div>
                      </div>
                    </div>

                    {fileData.sheets.length > 1 && (
                      <div className="flex items-center gap-2 text-[10px]">
                        <button
                          type="button"
                          onClick={() => handleSelectAllSheets(true)}
                          className="px-2 py-0.5 rounded bg-emerald-100 hover:bg-emerald-200 text-emerald-800 font-semibold"
                        >
                          Selecionar Todas
                        </button>
                        <button
                          type="button"
                          onClick={() => handleSelectAllSheets(false)}
                          className="px-2 py-0.5 rounded bg-white border border-emerald-200 hover:bg-emerald-100 text-emerald-800 font-semibold"
                        >
                          Desmarcar Todas
                        </button>
                      </div>
                    )}
                  </div>

                  {/* Worksheets list with target table mapping */}
                  <div className="space-y-2 max-h-56 overflow-y-auto pr-1">
                    {fileData.sheets.map(s => (
                      <div
                        key={s.sheetName}
                        onClick={() => handleToggleSheet(s.sheetName)}
                        className={`flex flex-wrap items-center justify-between gap-2 p-2.5 rounded-lg border cursor-pointer transition-all ${
                          s.selected
                            ? 'bg-white border-emerald-400 shadow-xs'
                            : 'bg-slate-50/80 border-slate-200 opacity-60'
                        }`}
                      >
                        <div className="flex items-center gap-2.5 min-w-0">
                          <input
                            type="checkbox"
                            checked={s.selected}
                            onChange={() => {}}
                            className="w-4 h-4 text-emerald-600 rounded border-slate-300 focus:ring-emerald-500"
                          />
                          <div className="min-w-0">
                            <div className="font-bold text-slate-800 text-xs flex items-center gap-1.5">
                              <span>{s.sheetName}</span>
                            </div>
                            <div className="text-[10px] text-slate-500">
                              {s.rows.length.toLocaleString('pt-BR')} registros • {s.columns.length} colunas
                            </div>
                          </div>
                        </div>

                        <div
                          className="flex items-center gap-1.5 flex-wrap"
                          onClick={(e) => e.stopPropagation()}
                        >
                          <select
                            value={s.targetMode === 'replace' && s.targetTableId ? `replace:${s.targetTableId}` : 'new'}
                            onChange={(e) => {
                              const val = e.target.value;
                              if (val.startsWith('replace:')) {
                                handleUpdateSheetMapping(s.sheetName, {
                                  selected: true,
                                  targetMode: 'replace',
                                  targetTableId: val.slice('replace:'.length),
                                });
                              } else {
                                handleUpdateSheetMapping(s.sheetName, {
                                  targetMode: 'new',
                                  targetTableId: '',
                                });
                              }
                            }}
                            className="text-[10px] border border-slate-300 rounded px-1.5 py-1 bg-slate-50 text-slate-800 font-semibold"
                          >
                            <option value="new">Criar nova tabela</option>
                            {tables.map(t => (
                              <option key={t.id} value={`replace:${t.id}`}>
                                Substituir: {t.name}
                              </option>
                            ))}
                          </select>

                          {s.targetMode === 'new' && (
                            <input
                              type="text"
                              value={s.tableName}
                              onChange={(e) =>
                                handleUpdateSheetMapping(s.sheetName, { tableName: e.target.value })
                              }
                              placeholder="Nome da tabela"
                              className="w-32 text-[10px] border border-slate-300 rounded px-1.5 py-1 bg-white text-slate-800 font-mono"
                            />
                          )}
                        </div>
                      </div>
                    ))}
                  </div>

                  <div className="flex items-center justify-between pt-2 border-t border-emerald-200/60">
                    <span className="text-[11px] text-emerald-800 font-medium">
                      {fileData.sheets.filter(s => s.selected).length} de {fileData.sheets.length} planilhas selecionadas
                    </span>
                    <button
                      type="button"
                      onClick={handleConfirmImportFile}
                      disabled={fileData.sheets.filter(s => s.selected).length === 0}
                      className="px-4 py-2 bg-emerald-600 hover:bg-emerald-700 disabled:opacity-50 text-white font-bold rounded-lg text-xs shadow-xs transition-colors cursor-pointer"
                    >
                      {fileData.sheets.some(s => s.selected && s.targetMode === 'replace')
                        ? 'Aplicar Atualização / Carregar no Modelo'
                        : fileData.sheets.filter(s => s.selected).length > 1
                        ? `Carregar ${fileData.sheets.filter(s => s.selected).length} Tabelas no Modelo`
                        : 'Carregar no Modelo'}
                    </button>
                  </div>
                </div>
              )}
            </div>
          )}

          {/* TAB 2: REST API */}
          {activeTab === 'api' && (
            <div className="space-y-3">
              <div>
                <label className="font-semibold text-slate-700 block mb-1">URL do Endpoint REST (JSON/CSV)</label>
                <input
                  type="text"
                  value={apiUrl}
                  onChange={(e) => setApiUrl(e.target.value)}
                  className="w-full border rounded-lg p-2 bg-slate-50 text-xs text-slate-800"
                />
              </div>

              <div className="p-3 bg-amber-50 rounded-lg border border-amber-200 text-amber-900 text-[11px] space-y-1">
                <div className="font-bold flex items-center gap-1">
                  <AlertCircle className="w-3.5 h-3.5" /> Tratamento de CORS no Navegador:
                </div>
                <p>
                  Requisições diretas do navegador exigem que o servidor de destino permita origens externas (cabeçalho <code>Access-Control-Allow-Origin: *</code>). Endpoints públicos retornando JSON podem ser carregados diretamente no modelo e atualizados via botão Atualizar.
                </p>
              </div>

              <div className="flex items-center gap-2 flex-wrap">
                <button
                  onClick={handleTestApi}
                  disabled={apiTesting}
                  className="px-4 py-2 bg-slate-900 hover:bg-slate-800 text-white font-bold rounded-lg cursor-pointer"
                >
                  {apiTesting && !apiImporting ? 'Testando...' : 'Testar Conexão REST'}
                </button>
                <button
                  onClick={() => void handleImportRestApiData()}
                  disabled={apiTesting || !apiUrl.trim()}
                  className="px-4 py-2 bg-emerald-600 hover:bg-emerald-700 disabled:opacity-50 text-white font-bold rounded-lg cursor-pointer"
                >
                  {apiImporting ? 'Carregando JSON...' : replaceTargetTable ? `Substituir "${replaceTargetTable.name}" via REST` : 'Importar Dados JSON no Modelo'}
                </button>
              </div>

              {apiResult && (
                <div className={`p-3 rounded-lg border text-xs ${apiResult.success ? 'bg-emerald-50 border-emerald-200 text-emerald-800' : 'bg-red-50 border-red-200 text-red-800'}`}>
                  {apiResult.message}
                </div>
              )}
            </div>
          )}

          {/* TAB 3: RELATIONAL DB & GATEWAY */}
          {activeTab === 'db' && (
            <div className="space-y-4">
              {/* Quick shortcut to mock postgres connector */}
              <div className="p-3 bg-amber-50 rounded-xl border border-amber-200 flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <span className="text-lg">🐘</span>
                  <div>
                    <div className="font-bold text-slate-900 text-xs">
                      Quer testar PostgreSQL imediatamente sem configurar um Gateway local?
                    </div>
                    <div className="text-[11px] text-slate-600">
                      Use nosso conector simulado com esquema <code className="font-mono bg-white px-1 py-0.2 rounded text-amber-900">public</code> completo e dados corporativos.
                    </div>
                  </div>
                </div>
                <button
                  type="button"
                  onClick={() => setActiveTab('postgres')}
                  className="px-3 py-1.5 bg-amber-500 hover:bg-amber-400 text-slate-950 font-bold rounded-lg text-xs shadow-xs transition-colors shrink-0 cursor-pointer"
                >
                  Abrir Mock PostgreSQL →
                </button>
              </div>

              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="font-semibold text-slate-700 dark:text-slate-200 block mb-1">SGBD / Banco de Dados</label>
                  <select
                    value={dbType}
                    onChange={(e) => setDbType(e.target.value as any)}
                    className="w-full border border-slate-300 dark:border-slate-700 rounded p-1.5 bg-slate-50 dark:bg-slate-800 text-slate-800 dark:text-slate-100"
                  >
                    <option value="sqlserver">Microsoft SQL Server</option>
                    <option value="postgres">PostgreSQL</option>
                    <option value="mysql">MySQL</option>
                    <option value="oracle">Oracle Database</option>
                  </select>
                </div>

                <div>
                  <label className="font-semibold text-slate-700 dark:text-slate-200 block mb-1">Ambiente de Execução</label>
                  <select
                    value={dbEnv}
                    onChange={(e) => setDbEnv(e.target.value as any)}
                    className="w-full border border-slate-300 dark:border-slate-700 rounded p-1.5 bg-slate-50 dark:bg-slate-800 text-slate-800 dark:text-slate-100"
                  >
                    <option value="dev">Desenvolvimento (DEV)</option>
                    <option value="test">Homologação (TEST)</option>
                    <option value="prod">Produção (PROD)</option>
                  </select>
                </div>

                <div>
                  <label className="font-semibold text-slate-700 block mb-1">Servidor / Host</label>
                  <input
                    type="text"
                    value={dbHost}
                    onChange={(e) => setDbHost(e.target.value)}
                    className="w-full border rounded p-1.5 bg-slate-50"
                  />
                </div>

                <div>
                  <label className="font-semibold text-slate-700 block mb-1">Nome do Banco de Dados</label>
                  <input
                    type="text"
                    value={dbDatabase}
                    onChange={(e) => setDbDatabase(e.target.value)}
                    className="w-full border rounded p-1.5 bg-slate-50"
                  />
                </div>
              </div>

              {/* Gateway Configuration */}
              <div className="p-3 bg-slate-50 rounded-xl border border-slate-200 space-y-2">
                <div className="flex items-center justify-between">
                  <div className="font-bold text-slate-800 flex items-center gap-1.5">
                    <Server className="w-4 h-4 text-amber-600" />
                    <span>Gateway de Dados On-Premises</span>
                  </div>
                  <input
                    type="checkbox"
                    checked={useGateway}
                    onChange={(e) => setUseGateway(e.target.checked)}
                    className="rounded text-amber-500"
                  />
                </div>
                <p className="text-[11px] text-slate-500">
                  Como este aplicativo é executado no navegador, a conexão com bancos SQL exige um proxy/gateway seguro corporativo.
                </p>
                {useGateway && (
                  <input
                    type="text"
                    value={gatewayUrl}
                    onChange={(e) => setGatewayUrl(e.target.value)}
                    placeholder="URL do Gateway (ex.: http://localhost:8080/gateway)"
                    className="w-full border rounded p-1.5 bg-white text-xs"
                  />
                )}
              </div>

              <div className="flex items-center gap-2">
                <button
                  onClick={handleTestDatabase}
                  disabled={dbTesting}
                  className="px-4 py-2 bg-slate-900 hover:bg-slate-800 text-white font-bold rounded-lg text-xs"
                >
                  {dbTesting ? 'Testando Conexão...' : 'Testar Conexão SQL'}
                </button>
                {dbResult?.success && (
                  <button
                    onClick={handleImportMockDbTable}
                    className="px-4 py-2 bg-emerald-600 hover:bg-emerald-700 text-white font-bold rounded-lg text-xs"
                  >
                    Importar Tabela Demonstrativa (tb_faturamento_erp)
                  </button>
                )}
              </div>

              {dbResult && (
                <div
                  className={`p-3 rounded-lg border text-xs ${
                    dbResult.success
                      ? 'bg-emerald-50 border-emerald-200 text-emerald-800'
                      : 'bg-red-50 border-red-200 text-red-800'
                  }`}
                >
                  {dbResult.message}
                </div>
              )}
            </div>
          )}

          {/* TAB 4: SAMPLES */}
          {activeTab === 'samples' && (
            <div className="space-y-3">
              <p className="text-slate-600 text-xs">
                Carregue conjuntos de dados corporativos brasileiros sintéticos já modelados com fatos, dimensões e medidas:
              </p>
              <div className="grid grid-cols-2 gap-3">
                <div className="p-3 border rounded-xl bg-slate-50 hover:border-amber-400 transition-colors">
                  <div className="font-bold text-slate-900 text-xs">Comercial & Vendas B2B</div>
                  <div className="text-[11px] text-slate-500 mt-1">90 transações, margens, clientes e vendedores</div>
                </div>
                <div className="p-3 border rounded-xl bg-slate-50 hover:border-amber-400 transition-colors">
                  <div className="font-bold text-slate-900 text-xs">DRE Gerencial & Finanças</div>
                  <div className="text-[11px] text-slate-500 mt-1">EBITDA, CMV, despesas operacionais e orçado vs realizado</div>
                </div>
                <div className="p-3 border rounded-xl bg-slate-50 hover:border-amber-400 transition-colors">
                  <div className="font-bold text-slate-900 text-xs">Logística & Frotas</div>
                  <div className="text-[11px] text-slate-500 mt-1">Rotas, consumo de diesel, pedágios e custo por km</div>
                </div>
                <div className="p-3 border rounded-xl bg-slate-50 hover:border-amber-400 transition-colors">
                  <div className="font-bold text-slate-900 text-xs">RH & Gestão de Pessoas</div>
                  <div className="text-[11px] text-slate-500 mt-1">Headcount, turnover, horas extras e absenteísmo</div>
                </div>
              </div>

              <button
                onClick={() => {
                  loadInitialSampleData();
                  setDataSourceModalOpen(false);
                }}
                className="w-full py-2 bg-amber-500 hover:bg-amber-600 text-slate-950 font-bold rounded-lg text-xs"
              >
                Recarregar Todas as Amostras
              </button>
            </div>
          )}
        </div>
      </div>
    </div>,
    document.body
  );
};
