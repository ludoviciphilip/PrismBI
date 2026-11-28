export type SupportedLanguage = 'pt-BR' | 'en-US' | 'es-ES';
export type SupportedCurrency = 'BRL' | 'USD' | 'EUR';

export interface CurrencyConfig {
  code: SupportedCurrency;
  symbol: string;
  label: string;
}

export interface LanguageConfig {
  code: SupportedLanguage;
  label: string;
  shortLabel: string;
  flag: string;
}

export interface Translations {
  common: {
    save: string;
    saving: string;
    savedAt: string;
    saveFailed: string;
    open: string;
    cancel: string;
    confirm: string;
    delete: string;
    clear: string;
    all: string;
    search: string;
    export: string;
    import: string;
    new: string;
    close: string;
    loading: string;
    noData: string;
    value: string;
    total: string;
    uncategorized: string;
    actions: string;
    language: string;
    currency: string;
  };
  topBar: {
    newProject: string;
    newProjectTooltip: string;
    openInsight: string;
    openInsightTooltip: string;
    exportInsight: string;
    exportInsightTooltip: string;
    copilotAi: string;
    confirmNewProjectTitle: string;
    confirmNewProjectMessage: string;
  };
  ribbonTabs: {
    home: string;
    insert: string;
    modeling: string;
    view: string;
    help: string;
  };
  ribbonActions: {
    getData: string;
    transformData: string;
    refresh: string;
    newMeasure: string;
    quickMeasure: string;
    newParameter: string;
    newVisual: string;
    textBox: string;
    themeManager: string;
    snapToGrid: string;
    filtersPane: string;
    visualsPane: string;
    fieldsPane: string;
    presentationMode: string;
    mobileLayout: string;
    desktopLayout: string;
    daxDocs: string;
    about: string;
    pageSize: string;
  };
  nav: {
    report: string;
    data: string;
    model: string;
    query: string;
    daxquery: string;
    metabase: string;
    dashboard: string;
    governance: string;
    daxcompliance?: string;
    limitations: string;
  };
  panes: {
    collapse: string;
    expand: string;
    filters: {
      title: string;
      onThisPage: string;
      onAllPages: string;
      clearAll: string;
      noFilters: string;
      searchFields: string;
      sortValues: string;
      sortAZ: string;
      sortZA: string;
      sortByCount: string;
    };
    visualizations: {
      title: string;
      buildVisual: string;
      formatVisual: string;
      types: string;
      xAxis: string;
      yAxis: string;
      legend: string;
      tooltip: string;
    };
    fields: {
      title: string;
      search: string;
      tables: string;
      newMeasure: string;
    };
  };
  visualCard: {
    moreOptions: string;
    sortBy: string;
    sortAscending: string;
    sortDescending: string;
    sortColumnHint: string;
    focusMode: string;
    restoreSize: string;
    deleteVisual: string;
    exportCsv: string;
    viewTable: string;
    viewChart: string;
    moveTooltip: string;
    noDataToDisplay: string;
    allRecords: string;
    selectField: string;
    drillthrough: string;
    openCalcModal: string;
  };
  /** v0.10 Rodada 2 (Frente G, seams i18n): controles de drill-down no cabeçalho do visual e menu de contexto. */
  drill: {
    controlsGroupLabel: string;
    drillUp: string;
    drillUpHint: string;
    drillUpAtTop: string;
    drillDownMode: string;
    drillDownModeOnHint: string;
    drillDownModeOffHint: string;
    nextLevel: string;
    nextLevelHint: string;
    expandAllLevels: string;
    expandAllLevelsHint: string;
    atLowestLevel: string;
    matrixControlsGroupLabel: string;
    expandOneLevel: string;
    expandOneLevelHint: string;
    expandOneLevelDoneHint: string;
    expandAll: string;
    expandAllHint: string;
    expandAllDoneHint: string;
    collapseAll: string;
    collapseAllHint: string;
    collapseAllDoneHint: string;
    pointMenuLabel: string;
    drillDownAction: string;
    drillDownActionHint: string;
    drillUpActionHint: string;
    showAsTable: string;
    showAsTableHint: string;
    drillthroughAction: string;
    drillthroughActionHint: string;
    drillthroughUnavailableHint: string;
    /** v0.11 Rodada 2 (Frente I): alterna qual eixo da matriz os controles de
     * drill/expandir afetam — paridade com "Linhas"/"Colunas" no Power BI Desktop. */
    axisRows: string;
    axisColumns: string;
    switchAxisHint: string;
  };
  slicer: {
    searchPlaceholder: string;
    selectAll: string;
    between: string;
    relativeDate: string;
    min: string;
    max: string;
    start: string;
    end: string;
    anchorLast: string;
    anchorThis: string;
    anchorNext: string;
    unitDays: string;
    unitWeeks: string;
    unitMonths: string;
    unitQuarters: string;
    unitYears: string;
    includeToday: string;
    syncSlicersTitle: string;
    syncGroup: string;
    syncGroupPlaceholder: string;
    syncPages: string;
    syncAllPages: string;
    filterLabelBetweenDates: string;
    filterLabelRelative: string;
    showSearchBox: string;
  };
  report: {
    addPage: string;
    duplicatePage: string;
    renamePage: string;
    deletePage: string;
    fitToPage: string;
    emptyPageTitle: string;
    emptyPageSubtitle: string;
    canvasSize: string;
  };
  /** v0.11 Frente D: painel Indicadores (Bookmarks). */
  bookmarks: {
    title: string;
    expandPanel: string;
    collapsePanel: string;
    add: string;
    groupAll: string;
    ungroupAll: string;
    play: string;
    previous: string;
    next: string;
    exit: string;
    empty: string;
    moreOptions: string;
    optionsOf: string;
    refresh: string;
    duplicate: string;
    delete: string;
    captureData: string;
    captureDisplay: string;
    captureCurrentPage: string;
    scopeAllVisuals: string;
    scopeSelectedVisuals: string;
    renameAria: string;
    newBookmarkPrefix: string;
    newGroupName: string;
  };
  /** v0.11 Frente D: painel Seleção. */
  selection: {
    title: string;
    expandPanel: string;
    collapsePanel: string;
    empty: string;
    showVisual: string;
    hideVisual: string;
  };
  /** v0.11 Frente D: Dicas de página (PageTooltipSettings/TooltipCard). */
  pageTooltip: {
    pageInfo: string;
    allowAsTooltip: string;
    hiddenInPresentation: string;
    fieldsLabel: string;
    noFieldsAvailable: string;
    tooltipLabel: string;
    type: string;
    typeDefault: string;
    typeReportPage: string;
    pageLabel: string;
    automatic: string;
    noTooltipPagesYet: string;
  };
  /** v0.11 Frente D: literais fixos da matriz hierárquica (HierarchicalMatrix). */
  matrix: {
    total: string;
    grandTotal: string;
    blank: string;
    rowsLabel: string;
  };
  /** v0.11 Frente D: editor de níveis de drill (DrillLevelsEditor). */
  drillEditor: {
    rowLevelsLabel: string;
    axisHierarchyLabel: string;
    level: string;
    removeLevel: string;
    addLevelAria: string;
    addLevelOption: string;
    addLevelTitle: string;
    chooseAxisFirstMatrix: string;
    chooseAxisFirstChart: string;
    autoDateHierarchy: string;
  };
  connectors: {
    tabLabel: string;
    nativeBadge: string;
    securityNotice: string;
    connect: string;
    connecting: string;
    disconnect: string;
    connected: string;
    browseTab: string;
    sqlTab: string;
    schemas: string;
    tables: string;
    targetTableName: string;
    runPreview: string;
    loadIntoModel: string;
    host: string;
    port: string;
    database: string;
    user: string;
    password: string;
    filePath: string;
    parquetUrl: string;
    selectTableOrWriteSql: string;
    importSuccess: string;
  };
  /** v0.11 (atalhos de teclado): toasts dos atalhos e diálogo "Atalhos de teclado". */
  shortcuts: {
    toastSaved: string;
    toastSaveFailed: string;
    toastExported: string;
    dialogTitle: string;
    dialogSubtitle: string;
    searchPlaceholder: string;
    noResults: string;
    menuItem: string;
    columnAction: string;
    columnShortcut: string;
  };
}
