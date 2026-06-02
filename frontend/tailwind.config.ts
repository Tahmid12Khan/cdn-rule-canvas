import type { Config } from "tailwindcss";

const config: Config = {
  content: ["./src/**/*.{ts,tsx}"],
  darkMode: "class",
  theme: {
    extend: {
      fontFamily: {
        sans: ["var(--font-inter)", "ui-sans-serif", "system-ui", "sans-serif"],
        mono: [
          "var(--font-mono)",
          "ui-monospace",
          "SFMono-Regular",
          "Menlo",
          "monospace",
        ],
      },
      borderRadius: {
        DEFAULT: "var(--radius)",
        sm: "4px",
        md: "var(--radius)",
        lg: "8px",
        xl: "10px",
      },
      colors: {
        // --- semantic surface/text/accent tokens (WS6) ---
        bg: {
          DEFAULT: "var(--bg)",
          elevated: "var(--bg-elevated)",
          overlay: "var(--bg-overlay)",
        },
        border: {
          DEFAULT: "var(--border)",
          strong: "var(--border-strong)",
        },
        fg: {
          DEFAULT: "var(--fg)",
          muted: "var(--fg-muted)",
          subtle: "var(--fg-subtle)",
        },
        accent: {
          DEFAULT: "var(--accent)",
          fg: "var(--accent-fg)",
          muted: "var(--accent-muted)",
          onMuted: "var(--accent-on-muted)",
        },
        danger: { DEFAULT: "var(--danger)", bg: "var(--danger-bg)" },
        success: "var(--success)",
        warning: "var(--warning)",

        brand: {
          DEFAULT: "var(--accent)",
          50: "var(--accent-muted)",
          100: "var(--accent-muted)",
          400: "#2dd4bf",
          500: "var(--accent)",
          600: "#0d9488",
          700: "#0f766e",
          900: "#134e4a",
        }, // teal — the single accent
        // action-* now aliases the single accent (drop the second blue accent).
        action: {
          DEFAULT: "var(--accent)",
          600: "var(--accent)",
          700: "#0d9488",
        },
        // The nav/* tokens are heavily used as general UI chrome — bridge them
        // to the semantic vars so TopNav + 30+ files re-theme with zero edits.
        nav: {
          DEFAULT: "var(--fg)",
          bg: "var(--bg-elevated)",
          border: "var(--border)",
          fg: "var(--fg)",
          muted: "var(--fg-muted)",
        },
        status: {
          live: "#16a34a",
          liveBg: "#0f2a1c",
          liveFg: "#4ec98a", // green pill
          staging: "#d97706",
          stagingBg: "#2e220f",
          stagingFg: "#e0a44a", // amber pill
          // prev/* doubled as generic UI tokens — bridge to semantic vars.
          prev: "var(--fg-subtle)",
          prevBg: "var(--border)",
          prevFg: "var(--fg-muted)",
          draft: "var(--fg-muted)",
          draftBorder: "var(--border-strong)", // outlined pill
        },
        node: {
          decision: "var(--accent)", // diamond fill/border
          start: "#0b0f17", // black entry marker — fixed (black in both themes)
          outcome: "#0b0f17", // black rectangle
          subrule: "#ec4899", // hot-pink
          action: "#f59e0b", // orange/amber
          yes: "var(--accent)", // YES branch (teal)
          no: "#6b7280", // NO branch (gray)
        },
      },
      backgroundImage: {
        // dotted-grid canvas bg — used by globals.css util when not using <Background variant="dots"/>
        "dot-grid": "radial-gradient(circle, var(--dot) 1px, transparent 1px)",
      },
      backgroundSize: { "dot-grid": "20px 20px" },
    },
  },
  plugins: [],
};

export default config;
