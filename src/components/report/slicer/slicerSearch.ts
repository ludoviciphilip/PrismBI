/** Accent/case-insensitive search for slicer item lists (Power BI "showSearch"). */
export function normalizeSearchText(s: string): string {
  return s
    .toLowerCase()
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '');
}

export function filterItemsBySearch(items: string[], query: string): string[] {
  if (!query.trim()) return items;
  const nq = normalizeSearchText(query);
  return items.filter(it => normalizeSearchText(it).includes(nq));
}
