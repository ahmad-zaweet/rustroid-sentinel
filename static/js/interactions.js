/**
 * Rustroid Sentinel — Interactive Effects
 * Sentinel Prime 2026
 */

// ============================================
// Spotlight Hover Effect — teal variant
// ============================================
function initSpotlightEffects() {
  const spotlightCards = document.querySelectorAll(
    ".spotlight-card, .stat-card, .chart-card",
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
    ".stat-card, .metric-card, .glass-card",
  );

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

  if (prefersReducedMotion()) {
    console.log("[sentinel] Reduced motion preference detected.");
  }
});

// ============================================
// Hazard Dropdown — UI only (HTMX sends the request)
// ============================================
document.addEventListener("click", (e) => {
  const dropdown = document.getElementById("hazard-dropdown");
  if (!dropdown) return;

  if (e.target.closest("#hazard-dropdown-trigger")) {
    toggleDropdown();
    return;
  }

  const option = e.target.closest(".hazard-option");
  if (option) {
    const value = option.dataset.value;
    selectOption(value);
    return;
  }

  if (!dropdown.contains(e.target)) {
    closeDropdown();
  }
});

function toggleDropdown() {
  const menu = document.getElementById("hazard-dropdown-menu");
  const trigger = document.getElementById("hazard-dropdown-trigger");
  const isOpen = menu.classList.contains("open");

  if (isOpen) {
    closeDropdown();
  } else {
    menu.classList.add("open");
    trigger.setAttribute("aria-expanded", "true");
  }
}

function closeDropdown() {
  const menu = document.getElementById("hazard-dropdown-menu");
  const trigger = document.getElementById("hazard-dropdown-trigger");
  if (!menu || !trigger) return;
  menu.classList.remove("open");
  trigger.setAttribute("aria-expanded", "false");
}

function selectOption(value) {
  const hiddenInput = document.getElementById("hazard-filter");
  const label = document.getElementById("hazard-dropdown-label");

  if (value === "") {
    hiddenInput.removeAttribute("name");
    hiddenInput.value = "";
  } else {
    hiddenInput.setAttribute("name", "hazard_class");
    hiddenInput.value = value;
  }

  label.textContent = value || "All Hazards";
  closeDropdown();

  hiddenInput.dispatchEvent(new Event("change", { bubbles: true }));
}

window.RustroidUI = {
  triggerUpdateAnimation,
  showSkeleton,
  showToast,
  prefersReducedMotion,
  escapeHtml,
};
