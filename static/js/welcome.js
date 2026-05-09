/**
 * Rustroid Sentinel — First-Visit Welcome Banner
 * Shows once per browser, saved to localStorage.
 */
(function () {
  const STORAGE_KEY = "sentinel_welcome_v1";
  const banner = document.getElementById("welcome-banner");
  if (!banner) return;

  // Already seen — stay hidden
  if (localStorage.getItem(STORAGE_KEY)) return;

  // First visit — reveal
  banner.classList.add("banner-visible");
  document.body.style.overflow = "hidden";

  // Initialize Lucide icons inside the banner
  if (window.lucide && window.lucide.createIcons) {
    window.lucide.createIcons();
  }

  function dismiss() {
    localStorage.setItem(STORAGE_KEY, "1");
    banner.classList.remove("banner-visible");
    banner.classList.add("banner-exit");
    document.body.style.overflow = "";
    setTimeout(() => {
      banner.style.display = "none";
      banner.classList.remove("banner-exit");
    }, 420);
  }

  const cta = document.getElementById("banner-cta");
  if (cta) cta.addEventListener("click", dismiss);

  document.addEventListener("keydown", function onKey(e) {
    if (e.key === "Escape") {
      dismiss();
      document.removeEventListener("keydown", onKey);
    }
  });
})();
