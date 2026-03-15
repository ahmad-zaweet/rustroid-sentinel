tailwind.config = {
  theme: {
    extend: {
      colors: {
        "space-950": "#0a0a0f",
        "space-900": "#12121a",
        "space-800": "#1a1a2e",
        "space-700": "#252542",
        "nebula-purple": "#a78bfa",
        "nebula-blue": "#60a5fa",
        "nebula-pink": "#f472b6",
        "nebula-cyan": "#22d3ee",
        "text-primary": "#ffffff",
        "text-secondary": "#e5e7eb",
        "text-tertiary": "#9ca3af",
        "text-muted": "#6b7280",
        "hazard-critical": "#f87171",
        "hazard-high": "#fb923c",
        "hazard-medium": "#facc15",
        "hazard-low": "#4ade80",
      },
      backdropBlur: {
        glass: "24px",
        "glass-lg": "40px",
      },
      boxShadow: {
        glass: "0 8px 32px 0 rgba(0, 0, 0, 0.5)",
        "glass-lg": "0 16px 48px 0 rgba(0, 0, 0, 0.6)",
        glow: "0 0 40px rgba(167, 139, 250, 0.35)",
        "glow-blue": "0 0 40px rgba(96, 165, 250, 0.35)",
        "glow-cyan": "0 0 40px rgba(34, 211, 238, 0.35)",
        "glow-green": "0 0 40px rgba(74, 222, 128, 0.35)",
        "glow-critical": "0 0 40px rgba(248, 113, 113, 0.35)",
        "glow-high": "0 0 40px rgba(251, 146, 60, 0.35)",
      },
      backgroundImage: {
        "gradient-radial": "radial-gradient(var(--tw-gradient-stops))",
        "gradient-conic":
          "conic-gradient(from 180deg at 50% 50%, var(--tw-gradient-stops))",
        "gradient-radial-glow":
          "radial-gradient(ellipse 80% 50% at 50% -20%, rgba(167, 139, 250, 0.15), transparent)",
      },
      animation: {
        shimmer: "shimmer 2s linear infinite",
        "pulse-slow": "pulse 3s cubic-bezier(0.4, 0, 0.6, 1) infinite",
        "fade-slide-up":
          "fadeSlideUp 0.6s cubic-bezier(0.16, 1, 0.3, 1) forwards",
        gradient: "gradientShift 3s ease infinite",
        float: "float 3s ease-in-out infinite",
        "spin-glow": "spinGlow 1.5s linear infinite",
        "fade-in": "fadeIn 0.4s ease-out forwards",
        "scale-in": "scaleIn 0.3s ease-out forwards",
        "slide-in-right": "slideInRight 0.4s ease-out forwards",
      },
      keyframes: {
        shimmer: {
          "0%": { backgroundPosition: "-200% 0" },
          "100%": { backgroundPosition: "200% 0" },
        },
        fadeSlideUp: {
          "0%": { opacity: "0", transform: "translateY(24px)" },
          "100%": { opacity: "1", transform: "translateY(0)" },
        },
        gradientShift: {
          "0%, 100%": {
            backgroundSize: "200% 200%",
            backgroundPosition: "left center",
          },
          "50%": {
            backgroundSize: "200% 200%",
            backgroundPosition: "right center",
          },
        },
        float: {
          "0%, 100%": { transform: "translateY(0)" },
          "50%": { transform: "translateY(-4px)" },
        },
        spinGlow: {
          "0%, 100%": {
            transform: "rotate(0deg)",
            boxShadow: "0 0 8px rgba(167, 139, 250, 0.4)",
          },
          "50%": { boxShadow: "0 0 20px rgba(167, 139, 250, 0.6)" },
        },
        fadeIn: {
          "0%": { opacity: "0" },
          "100%": { opacity: "1" },
        },
        scaleIn: {
          "0%": { opacity: "0", transform: "scale(0.95)" },
          "100%": { opacity: "1", transform: "scale(1)" },
        },
        slideInRight: {
          "0%": { opacity: "0", transform: "translateX(20px)" },
          "100%": { opacity: "1", transform: "translateX(0)" },
        },
      },
      fontFamily: {
        heading: ["Rajdhani", "sans-serif"],
        body: ["Titillium Web", "sans-serif"],
        mono: ["Share Tech Mono", "monospace"],
      },
      borderRadius: {
        "4xl": "2.5rem",
      },
      scale: {
        102: "1.02",
        105: "1.05",
      },
    },
  },
  plugins: [],
};
