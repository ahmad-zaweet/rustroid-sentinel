/**
 * Rustroid Sentinel — Interactive Effects
 * Sentinel Prime 2026
 */

// ============================================
// Spotlight Hover Effect — teal variant
// ============================================
function initSpotlightEffects() {
  if (prefersReducedMotion()) return;

  const spotlightCards = document.querySelectorAll(
    ".spotlight-card, .stat-card-gradient, .chart-card, .similar-card-wrap, .active-hazard-wrap",
  );

  spotlightCards.forEach((card) => {
    const spotlight = card.querySelector(".spotlight");
    if (!spotlight) return;

    card.addEventListener("mousemove", (e) => {
      const rect = card.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;

      spotlight.style.setProperty("--mouse-x", `${x}px`);
      spotlight.style.setProperty("--mouse-y", `${y}px`);

      spotlight.style.background = `radial-gradient(
        380px circle at ${x}px ${y}px,
        rgba(45, 212, 191, 0.07),
        transparent 40%
      )`;
    });

    card.addEventListener("mouseleave", () => {
      spotlight.style.opacity = "0";
    });

    card.addEventListener("mouseenter", () => {
      spotlight.style.opacity = "1";
    });
  });
}

// ============================================
// Staggered Entrance Animation (IntersectionObserver)
// ============================================
function animateEntrance() {
  const elements = document.querySelectorAll(
    ".stat-card-gradient, .metric-card, .glass-card",
  );

  if (prefersReducedMotion()) {
    elements.forEach((el) => el.classList.add("animate-fade-slide-up"));
    return;
  }

  const observer = new IntersectionObserver(
    (entries) => {
      entries.forEach((entry, index) => {
        if (entry.isIntersecting) {
          const delayMs = Math.min(index * 80, 500);
          entry.target.style.animationDelay = `${delayMs}ms`;
          entry.target.classList.add("animate-fade-slide-up");
          observer.unobserve(entry.target);
        }
      });
    },
    {
      threshold: 0.08,
      rootMargin: "0px 0px -40px 0px",
    },
  );

  elements.forEach((el) => observer.observe(el));
}

// ============================================
// SSE Update Animation
// ============================================
function triggerUpdateAnimation(elementId) {
  const element = document.getElementById(elementId);
  if (!element) return;

  element.classList.add("animate-update-flash");
  setTimeout(() => {
    element.classList.remove("animate-update-flash");
  }, 600);
}

// ============================================
// Loading Skeleton
// ============================================
function showSkeleton(containerId, count = 3) {
  const container = document.getElementById(containerId);
  if (!container) return;

  container.innerHTML = Array(count)
    .fill(`<div class="skeleton h-16 rounded-xl mb-2.5"></div>`)
    .join("");
}

// ============================================
// Reduced Motion Detection
// ============================================
function prefersReducedMotion() {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

// ============================================
// Toast Notification System
// ============================================
function showToast(message, type = "info") {
  const existing = document.getElementById("toast-notification");
  if (existing) existing.remove();

  const typeClasses = {
    info: "bg-nebula-blue/90 border-nebula-blue",
    success: "bg-hazard-low/90 border-hazard-low",
    error: "bg-hazard-critical/90 border-hazard-critical",
    warning: "bg-hazard-medium/90 border-hazard-medium",
  };

  const typeIcons = {
    info: "ℹ️",
    success: "✅",
    error: "❌",
    warning: "⚠️",
  };

  const toast = document.createElement("div");
  toast.id = "toast-notification";
  toast.className = `fixed bottom-6 right-6 ${typeClasses[type]} text-white px-5 py-3.5 rounded-xl shadow-glass font-body font-medium text-sm z-50 backdrop-blur-glass border animate-fade-slide-up`;
  toast.innerHTML = `
    <div class="flex items-center gap-3">
      <span>${typeIcons[type]}</span>
      <span>${escapeHtml(message)}</span>
    </div>
  `;

  document.body.appendChild(toast);

  setTimeout(() => {
    toast.style.opacity = "0";
    toast.style.transform = "translateY(14px)";
    toast.style.transition = "all 0.28s ease";
    setTimeout(() => toast.remove(), 300);
  }, 5000);
}

// ============================================
// Utility: Escape HTML
// ============================================
function escapeHtml(text) {
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}

// ============================================
// Initialize on DOM Ready
// ============================================
document.addEventListener("DOMContentLoaded", () => {
  initSpotlightEffects();
  animateEntrance();
  initSseConnectionState();
  initMobileNav();

  if (prefersReducedMotion()) {
    console.log("[sentinel] Reduced motion preference detected.");
  }
});

// ============================================
// Mobile Nav Drawer
// ============================================
function initMobileNav() {
  const toggle = document.getElementById("mobile-nav-toggle");
  const drawer = document.getElementById("mobile-nav-drawer");
  const iconOpen = document.getElementById("mobile-nav-icon-open");
  const iconClose = document.getElementById("mobile-nav-icon-close");
  if (!toggle || !drawer) return;

  const close = () => {
    drawer.classList.add("hidden");
    drawer.classList.remove("flex");
    toggle.setAttribute("aria-expanded", "false");
    iconOpen?.classList.remove("hidden");
    iconClose?.classList.add("hidden");
  };

  const open = () => {
    drawer.classList.remove("hidden");
    drawer.classList.add("flex");
    toggle.setAttribute("aria-expanded", "true");
    iconOpen?.classList.add("hidden");
    iconClose?.classList.remove("hidden");
  };

  toggle.addEventListener("click", () => {
    const isOpen = toggle.getAttribute("aria-expanded") === "true";
    isOpen ? close() : open();
  });

  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") close();
  });

  drawer.querySelectorAll("a").forEach((a) => a.addEventListener("click", close));

  document.addEventListener("click", (e) => {
    if (toggle.getAttribute("aria-expanded") !== "true") return;
    if (!drawer.contains(e.target) && !toggle.contains(e.target)) close();
  });
}

// ============================================
// SSE Connection State — header live indicator + toast
// ============================================
function initSseConnectionState() {
  const container = document.getElementById("approaches-container");
  const dot = document.getElementById("connection-status");
  const label = document.getElementById("status-text");
  if (!container || !dot || !label) return;

  let isDown = false;

  const markDown = () => {
    if (isDown) return;
    isDown = true;
    dot.classList.remove("bg-hazard-low", "shadow-[0_0_10px_rgba(34,197,94,0.9)]");
    dot.classList.add("bg-hazard-critical", "shadow-[0_0_10px_rgba(239,68,68,0.9)]");
    label.textContent = "RECONNECTING";
    label.classList.remove("text-hazard-low");
    label.classList.add("text-hazard-critical");
    showToast("Live feed disconnected — retrying...", "warning");
  };

  const markUp = () => {
    if (!isDown) return;
    isDown = false;
    dot.classList.remove("bg-hazard-critical", "shadow-[0_0_10px_rgba(239,68,68,0.9)]");
    dot.classList.add("bg-hazard-low", "shadow-[0_0_10px_rgba(34,197,94,0.9)]");
    label.textContent = "SSE LIVE";
    label.classList.remove("text-hazard-critical");
    label.classList.add("text-hazard-low");
  };

  container.addEventListener("htmx:sseError", markDown);
  container.addEventListener("htmx:sseClose", markDown);
  container.addEventListener("htmx:afterSwap", () => {
    if (isDown) markUp();
  });
}

window.RustroidUI = {
  triggerUpdateAnimation,
  showSkeleton,
  showToast,
  prefersReducedMotion,
  escapeHtml,
};
