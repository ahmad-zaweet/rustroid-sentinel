/**
 * Rustroid Sentinel — First-Visit Mission Briefing Modal
 * Shows once per browser, saved to localStorage.
 */
(function () {
  const STORAGE_KEY = "sentinel_welcome_v2";
  const banner = document.getElementById("welcome-banner");
  if (!banner) return;

  // Already seen — ensure hidden
  if (localStorage.getItem(STORAGE_KEY)) {
    banner.style.display = "none";
    return;
  }

  // Blocks wheel/touchpad scroll directly — `overflow: hidden` on body alone
  // doesn't stop scroll-chaining to the documentElement on some browsers.
  function blockScroll(e) {
    e.preventDefault();
  }

  // First visit — reveal modal
  banner.style.display = "flex";
  document.body.style.overflow = "hidden";
  window.addEventListener("wheel", blockScroll, { passive: false });
  window.addEventListener("touchmove", blockScroll, { passive: false });

  function dismiss() {
    localStorage.setItem(STORAGE_KEY, "1");
    banner.style.opacity = "0";
    banner.style.transition = "opacity 0.3s ease";
    document.body.style.overflow = "";
    window.removeEventListener("wheel", blockScroll);
    window.removeEventListener("touchmove", blockScroll);
    setTimeout(() => {
      banner.style.display = "none";
    }, 300);
  }

  const cta = document.getElementById("banner-cta");
  if (cta) cta.addEventListener("click", dismiss);

  banner.addEventListener("click", function (e) {
    if (e.target === banner) dismiss();
  });

  document.addEventListener("keydown", function onKey(e) {
    if (e.key === "Escape") {
      dismiss();
      document.removeEventListener("keydown", onKey);
    }
  });
})();
