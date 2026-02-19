import { useState, useRef, useEffect, useCallback } from "react";

interface InfoPopoverProps {
  /** The content displayed inside the popover. */
  content: string;
  /** Accessible label for the trigger button. */
  ariaLabel?: string;
}

/**
 * A small (?) icon button that toggles a floating popover on click.
 * Dismisses when clicking outside. Accessible with aria-expanded / aria-haspopup.
 */
export default function InfoPopover({
  content,
  ariaLabel = "More information",
}: InfoPopoverProps) {
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLSpanElement>(null);

  const toggle = useCallback(() => setOpen((prev) => !prev), []);

  // Close on outside click
  useEffect(() => {
    if (!open) return;

    function handleClickOutside(e: MouseEvent) {
      if (
        containerRef.current &&
        !containerRef.current.contains(e.target as Node)
      ) {
        setOpen(false);
      }
    }

    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [open]);

  return (
    <span ref={containerRef} className="relative inline-flex items-center">
      <button
        type="button"
        onClick={toggle}
        aria-expanded={open}
        aria-haspopup="true"
        aria-label={ariaLabel}
        className="ml-1 inline-flex h-4 w-4 items-center justify-center rounded-full bg-border text-[10px] font-bold leading-none text-text-secondary hover:bg-text-muted hover:text-text-primary transition-colors"
      >
        ?
      </button>
      {open && (
        <div
          role="tooltip"
          className="absolute bottom-full left-1/2 z-50 mb-2 w-72 -translate-x-1/2 rounded-lg bg-surface-alt border border-border p-3 text-xs leading-relaxed text-text-secondary shadow-lg"
        >
          {content}
        </div>
      )}
    </span>
  );
}
