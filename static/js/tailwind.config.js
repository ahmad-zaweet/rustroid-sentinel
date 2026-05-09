tailwind.config = {
  theme: {
    extend: {
      colors: {
        // Deep navy space backgrounds (rebrand: no purple tint)
        "space-950": "#020B15",
        "space-900": "#040D18",
        "space-800": "#0C1B30",
        "space-700": "#162040",

        // Nebula accents (rebrand: stronger violet, teal-shifted cyan, amber replaces pink)
        "nebula-purple": "#7C3AED",
        "nebula-blue": "#38BDF8",
        "nebula-pink": "#F59E0B",   // rebrand: amber accent
        "nebula-cyan": "#2DD4BF",   // rebrand: warm teal

        // Semantic sentinel tokens
        "sentinel-teal": "#14B8A6",
        "sentinel-amber": "#F59E0B",
        "sentinel-violet": "#7C3AED",

        // Text hierarchy
        "text-primary": "#F1F5F9",
        "text-secondary": "#CBD5E1",
        "text-tertiary": "#94A3B8",
        "text-muted": "#64748B",

        // Hazard classification (semantic — never change these)
        "hazard-critical": "#EF4444",
        "hazard-high": "#F97316",
        "hazard-medium": "#EAB308",
        "hazard-low": "#22C55E",
      },
      backdropBlur: {
        glass: "24px",
        "glass-lg": "40px",
      },
      boxShadow: {
        glass: "0 8px 32px 0 rgba(0, 0, 0, 0.6)",
        "glass-lg": "0 16px 48px 0 rgba(0, 0, 0, 0.7)",
        glow: "0 0 40px rgba(124, 58, 237, 0.4)",
        "glow-blue": "0 0 40px rgba(56, 189, 248, 0.35)",
        "glow-cyan": "0 0 40px rgba(45, 212, 191, 0.4)",
        "glow-green": "0 0 40px rgba(34, 197, 94, 0.35)",
        "glow-critical": "0 0 40px rgba(239, 68, 68, 0.4)",
        "glow-high": "0 0 40px rgba(249, 115, 22, 0.35)",
        "glow-teal": "0 0 40px rgba(20, 184, 166, 0.4)",
        "glow-amber": "0 0 40px rgba(245, 158, 11, 0.4)",
      },
      backgroundImage: {
        "gradient-radial": "radial-gradient(var(--tw-gradient-stops))",
        "gradient-conic":
          "conic-gradient(from 180deg at 50% 50%, var(--tw-gradient-stops))",
        "gradient-radial-glow":
          "radial-gradient(ellipse 80% 50% at 50% -20%, rgba(124, 58, 237, 0.1), transparent)",
      },
      animation: {
        shimmer: "shimmer 2s linear infinite",
        "pulse-slow": "pulse 3s cubic-bezier(0.4, 0, 0.6, 1) infinite",
        "fade-slide-up":
          "fadeSlideUp 0.6s cubic-bezier(0.16, 1, 0.3, 1) forwards",
        gradient: "gradientShift 4s ease infinite",
        float: "float 4s ease-in-out infinite",
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
          "0%": { opacity: "0", transform: "translateY(18px)" },
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
          "50%": { transform: "translateY(-5px)" },
        },
        spinGlow: {
          "0%, 100%": {
            transform: "rotate(0deg)",
            boxShadow: "0 0 8px rgba(124, 58, 237, 0.4)",
          },
          "50%": { boxShadow: "0 0 22px rgba(124, 58, 237, 0.65)" },
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
        heading: ["Exo 2", "sans-serif"],
        body: ["Nunito Sans", "sans-serif"],
        mono: ["JetBrains Mono", "monospace"],
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
