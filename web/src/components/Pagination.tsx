import { ChevronLeft, ChevronRight } from "lucide-react";
import { Button } from "@/components/ui/button";
import { paginationPageItems } from "@/lib/pagination";

type PaginationProps = {
  label: string;
  page: number;
  totalPages: number;
  startItem: number;
  endItem: number;
  totalItems: number;
  onPageChange: (page: number) => void;
};

export function Pagination({
  label,
  page,
  totalPages,
  startItem,
  endItem,
  totalItems,
  onPageChange,
}: PaginationProps) {
  if (totalPages <= 1) {
    return null;
  }

  return (
    <nav className="list-pagination" aria-label={label}>
      <span>
        {startItem}–{endItem} / {totalItems}
      </span>
      <div className="pagination-buttons">
        <Button
          size="icon"
          variant="ghost"
          className="pagination-button"
          aria-label="上一页"
          disabled={page === 1}
          onClick={() => onPageChange(Math.max(1, page - 1))}
        >
          <ChevronLeft size={15} />
        </Button>
        {paginationPageItems(page, totalPages).map((item, index) =>
          item === "ellipsis" ? (
            <span className="pagination-ellipsis" aria-hidden="true" key={`ellipsis-${index}`}>
              …
            </span>
          ) : (
            <Button
              key={item}
              size="icon"
              variant={item === page ? "secondary" : "ghost"}
              className={`pagination-button${item === page ? " is-active" : ""}`}
              aria-label={`第 ${item} 页`}
              aria-current={item === page ? "page" : undefined}
              onClick={() => onPageChange(item)}
            >
              {item}
            </Button>
          ),
        )}
        <Button
          size="icon"
          variant="ghost"
          className="pagination-button"
          aria-label="下一页"
          disabled={page === totalPages}
          onClick={() => onPageChange(Math.min(totalPages, page + 1))}
        >
          <ChevronRight size={15} />
        </Button>
      </div>
    </nav>
  );
}
