export type PaginationResult<T> = {
  items: T[];
  page: number;
  pageSize: number;
  totalItems: number;
  totalPages: number;
  startItem: number;
  endItem: number;
};

export function paginate<T>(items: T[], requestedPage: number, pageSize: number): PaginationResult<T> {
  const normalizedPageSize = Math.max(1, Math.floor(pageSize));
  const totalPages = Math.max(1, Math.ceil(items.length / normalizedPageSize));
  const page = Math.min(totalPages, Math.max(1, Math.floor(requestedPage)));
  const startIndex = (page - 1) * normalizedPageSize;
  const pageItems = items.slice(startIndex, startIndex + normalizedPageSize);

  return {
    items: pageItems,
    page,
    pageSize: normalizedPageSize,
    totalItems: items.length,
    totalPages,
    startItem: items.length === 0 ? 0 : startIndex + 1,
    endItem: startIndex + pageItems.length,
  };
}

export function paginationPageItems(
  requestedPage: number,
  requestedTotalPages: number,
): Array<number | "ellipsis"> {
  const totalPages = Math.max(1, Math.floor(requestedTotalPages));
  const page = Math.min(totalPages, Math.max(1, Math.floor(requestedPage)));
  if (totalPages <= 7) {
    return Array.from({ length: totalPages }, (_, index) => index + 1);
  }

  const visiblePages = new Set([1, totalPages, page - 1, page, page + 1]);
  const sortedPages = Array.from(visiblePages)
    .filter((item) => item >= 1 && item <= totalPages)
    .sort((left, right) => left - right);
  const items: Array<number | "ellipsis"> = [];
  for (const visiblePage of sortedPages) {
    const previous = items[items.length - 1];
    if (typeof previous === "number" && visiblePage - previous > 1) {
      items.push("ellipsis");
    }
    items.push(visiblePage);
  }
  return items;
}
