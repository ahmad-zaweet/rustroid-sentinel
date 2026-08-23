/**
 * Rustroid Sentinel — Dashboard & Atmospheric Space Canvas
 * Claude Design v2 Architecture
 */

let velocityChart = null;

// Palette Tokens
const PALETTE = {
  violet: "#7C3AED",
  blue: "#38BDF8",
  teal: "#2DD4BF",
  tealBright: "#4FE8DC",
  lilac: "#C4B5FD",
  hazardCritical: "#EF4444",
  hazardHigh: "#F97316",
  hazardMedium: "#EAB308",
  hazardLow: "#22C55E",
};

/**
 * Atmospheric 2D Starfield Canvas with Mouse Parallax & Twinkle
 */
function initStarsCanvas() {
  const cv = document.getElementById("stars-canvas");
  if (!cv) return;
  const ctx = cv.getContext("2d");
  if (!ctx) return;

  let stars = [];
  let raf;
  let w = 0, h = 0;
  let mx = 0, my = 0;
  let tx = 0, ty = 0;

  const resize = () => {
    const dpr = window.devicePixelRatio || 1;
    w = window.innerWidth;
    h = window.innerHeight;
    cv.width = w * dpr;
    cv.height = h * dpr;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    const count = Math.round((w * h) / 7200);
    stars = Array.from({ length: count }, () => ({
      x: Math.random() * w,
      y: Math.random() * h,
      r: Math.random() * 1.2 + 0.25,
      z: 0.3 + Math.random() * 1,
      p: Math.random() * 6.2832,
      s: 0.4 + Math.random() * 1.1,
      c: Math.random() > 0.84 ? "165,243,235" : (Math.random() > 0.66 ? "196,181,253" : "236,233,248"),
    }));
  };

  const FRAME_INTERVAL = 1000 / 30;
  let lastFrame = 0;

  const draw = (t) => {
    if (t - lastFrame < FRAME_INTERVAL) {
      raf = requestAnimationFrame(draw);
      return;
    }
    lastFrame = t;

    tx += (mx - tx) * 0.045;
    ty += (my - ty) * 0.045;
    ctx.clearRect(0, 0, w, h);

    for (let i = 0; i < stars.length; i++) {
      const s = stars[i];
      const a = 0.14 + 0.5 * (0.5 + 0.5 * Math.sin((t / 1300) * s.s + s.p));
      ctx.fillStyle = `rgba(${s.c},${a})`;
      ctx.beginPath();
      ctx.arc(s.x + tx * s.z, s.y + ty * s.z, s.r, 0, 6.2832);
      ctx.fill();
    }
    if (!document.hidden) raf = requestAnimationFrame(draw);
  };

  window.addEventListener("resize", resize);
  window.addEventListener("mousemove", (e) => {
    mx = (e.clientX / window.innerWidth - 0.5) * -26;
    my = (e.clientY / window.innerHeight - 0.5) * -18;
  });

  document.addEventListener("visibilitychange", () => {
    if (document.hidden) {
      cancelAnimationFrame(raf);
    } else if (!reducedMotionActive) {
      raf = requestAnimationFrame(draw);
    }
  });

  resize();

  const reducedMotionActive =
    window.RustroidUI?.prefersReducedMotion?.() ??
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  if (reducedMotionActive) {
    draw(0);
  } else {
    raf = requestAnimationFrame(draw);
  }
}

/**
 * Spotlight Cursor Tracking for Active Hazard Card
 */
function initHazardSpotlight() {
  const box = document.getElementById("active-hazard-box");
  const spot = document.getElementById("hazard-spotlight");
  if (!box || !spot) return;

  box.addEventListener("mousemove", (e) => {
    const r = box.getBoundingClientRect();
    const x = e.clientX - r.left;
    const y = e.clientY - r.top;
    spot.style.transform = `translate(${x}px, ${y}px)`;
    spot.style.opacity = "1";
  });

  box.addEventListener("mouseleave", () => {
    spot.style.opacity = "0";
  });
}

/**
 * Initialize Chart.js Velocity Trajectory Chart
 */
function initVelocityChart() {
  const canvas = document.getElementById("velocityChart");
  if (!canvas) return;
  const ctx = canvas.getContext("2d");

  const gradient = ctx.createLinearGradient(0, 0, 0, 320);
  gradient.addColorStop(0, "rgba(79, 232, 220, 0.35)");
  gradient.addColorStop(0.55, "rgba(124, 58, 237, 0.12)");
  gradient.addColorStop(1, "rgba(124, 58, 237, 0.0)");

  velocityChart = new Chart(ctx, {
    type: "line",
    data: {
      labels: [],
      datasets: [
        {
          label: "Velocity (km/h)",
          data: [],
          borderColor: PALETTE.tealBright,
          backgroundColor: gradient,
          borderWidth: 2.25,
          fill: true,
          tension: 0.38,
          pointRadius: 3.5,
          pointHoverRadius: 7,
          pointBackgroundColor: PALETTE.lilac,
          pointBorderColor: "#050410",
          pointBorderWidth: 2,
          pointHoverBackgroundColor: "#FFFFFF",
          pointHoverBorderColor: PALETTE.tealBright,
        },
      ],
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      interaction: {
        intersect: false,
        mode: "index",
      },
      plugins: {
        legend: { display: false },
        tooltip: {
          backgroundColor: "rgba(14, 10, 30, 0.96)",
          titleColor: "#F4F1FC",
          bodyColor: "#C3BEDA",
          borderColor: "rgba(196, 181, 253, 0.2)",
          borderWidth: 1,
          padding: 12,
          cornerRadius: 10,
          titleFont: { size: 12, family: "'Exo 2', sans-serif", weight: 600 },
          bodyFont: { size: 11, family: "'JetBrains Mono', monospace" },
          displayColors: false,
          callbacks: {
            label: (item) => `Velocity: ${Math.round(item.parsed.y).toLocaleString()} km/h`,
          },
        },
      },
      scales: {
        x: {
          grid: { color: "rgba(196, 181, 253, 0.05)", drawBorder: false },
          ticks: {
            color: "#7B7499",
            font: { size: 9.5, family: "'JetBrains Mono', monospace" },
          },
        },
        y: {
          grid: { color: "rgba(196, 181, 253, 0.06)", drawBorder: false },
          ticks: {
            color: "#7B7499",
            font: { size: 9.5, family: "'JetBrains Mono', monospace" },
            callback: (v) => (v >= 1000 ? `${Math.round(v / 1000)}K` : v),
          },
          beginAtZero: false,
        },
      },
    },
  });
}

/**
 * Update Chart with Grouped Velocity Data
 */
function updateChart(velocityData) {
  if (!velocityChart) return;
  if (!velocityData || velocityData.length === 0) {
    velocityChart.data.labels = ["No Data"];
    velocityChart.data.datasets[0].data = [0];
    velocityChart.update("none");
    return;
  }

  const grouped = velocityData.reduce((acc, d) => {
    const date = new Date(d.date);
    const key = date.toLocaleDateString("en-US", { month: "short", day: "numeric" });
    if (!acc[key]) acc[key] = { total: 0, count: 0, date };
    acc[key].total += d.velocity_km_per_h;
    acc[key].count += 1;
    return acc;
  }, {});

  const sorted = Object.entries(grouped)
    .map(([label, data]) => ({ label, velocity: data.total / data.count, date: data.date }))
    .sort((a, b) => a.date - b.date);

  velocityChart.data.labels = sorted.map((d) => d.label.toUpperCase());
  velocityChart.data.datasets[0].data = sorted.map((d) => d.velocity);
  velocityChart.update("none");
}

/**
 * Live UTC Clock
 */
function updateUtcClock() {
  const el = document.getElementById("utc-clock");
  if (!el) return;
  const now = new Date();
  const dateStr = now.toLocaleDateString("en-GB", {
    day: "2-digit",
    month: "short",
    year: "numeric",
  }).toUpperCase();
  const timeStr = now.toLocaleTimeString("en-GB", {
    hour: "2-digit",
    minute: "2-digit",
    timeZone: "UTC",
  });
  el.textContent = `${dateStr} · ${timeStr} UTC`;
}

// HTMX Swap Listener for velocity chart updates
document.addEventListener("htmx:afterSwap", (event) => {
  if (event.detail.target.id === "velocity-chart-container") {
    const chartDataEl = document.getElementById("velocity-chart-data");
    if (chartDataEl) {
      try {
        const labels = JSON.parse(chartDataEl.getAttribute("data-labels") || "[]");
        const values = JSON.parse(chartDataEl.getAttribute("data-values") || "[]");
        const velocityData = labels.map((label, idx) => ({
          date: new Date(`2026-${label}`),
          velocity_km_per_h: values[idx],
        }));
        updateChart(velocityData);
      } catch (err) {
        console.error("Failed to parse velocity data:", err);
      }
    }
  }

  // Re-bind spotlight if active hazard box swapped
  initHazardSpotlight();
});

// Initialization
document.addEventListener("DOMContentLoaded", () => {
  initStarsCanvas();
  initHazardSpotlight();
  initVelocityChart();

  if (window.VELOCITY_DATA && window.VELOCITY_DATA.length > 0) {
    updateChart(window.VELOCITY_DATA);
  }

  updateUtcClock();
  setInterval(updateUtcClock, 10000);
});
