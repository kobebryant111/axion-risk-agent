/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        axiom: {
          bg: "#f3f0f8",
          surface: "#ffffff",
          accent: "#7c5cfc",
          soft: "#ece7ff",
          muted: "#9ca3af",
          text: "#1f2937",
          danger: "#ef4444",
          warn: "#f59e0b",
          ok: "#22c55e",
          info: "#3b82f6",
        },
      },
      borderRadius: {
        "2xl": "1.25rem",
        "3xl": "1.5rem",
      },
      boxShadow: {
        soft: "0 8px 24px rgba(31, 41, 55, 0.06)",
      },
      fontFamily: {
        sans: [
          "Plus Jakarta Sans",
          "SF Pro Display",
          "PingFang SC",
          "Helvetica Neue",
          "Arial",
          "sans-serif",
        ],
      },
    },
  },
  plugins: [],
};
