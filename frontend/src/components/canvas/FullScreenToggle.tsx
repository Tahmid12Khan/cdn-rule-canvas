"use client";

// Full-screen toggle for the canvas, anchored bottom-right. Uses the Fullscreen
// API on the canvas container element. (Task 11)
import { useCallback, useEffect, useState } from "react";

interface FullScreenToggleProps {
  targetRef: React.RefObject<HTMLElement>;
}

export function FullScreenToggle({ targetRef }: FullScreenToggleProps) {
  const [isFullscreen, setIsFullscreen] = useState(false);

  useEffect(() => {
    const onChange = () =>
      setIsFullscreen(document.fullscreenElement === targetRef.current);
    document.addEventListener("fullscreenchange", onChange);
    return () => document.removeEventListener("fullscreenchange", onChange);
  }, [targetRef]);

  const toggle = useCallback(() => {
    const el = targetRef.current;
    if (!el) return;
    if (document.fullscreenElement) {
      void document.exitFullscreen();
    } else if (el.requestFullscreen) {
      void el.requestFullscreen();
    }
  }, [targetRef]);

  return (
    <button
      type="button"
      onClick={toggle}
      aria-label={isFullscreen ? "Exit full screen" : "Enter full screen"}
      className="flex h-9 w-9 items-center justify-center rounded-md border border-status-prevBg bg-bg-elevated text-status-prevFg shadow-sm hover:bg-brand-50"
    >
      <svg
        aria-hidden
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        className="h-4 w-4"
      >
        {isFullscreen ? (
          <path d="M9 9H5V5m10 0v4h4M9 15H5v4m10 0v-4h4" strokeLinecap="round" />
        ) : (
          <path
            d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5"
            strokeLinecap="round"
          />
        )}
      </svg>
    </button>
  );
}
