/**
 * Rustroid Sentinel Dashboard
 * Sentinel Prime 2026 — Deep Navy · Teal · Violet
 */

let velocityChart = null;
let currentVelocityPeriod = "7d";

// Sentinel Prime color palette
const colors = {
  nebulaPurple: "#7C3AED",
  nebulaBlue: "#38BDF8",
  nebulaCyan: "#2DD4BF",
  nebulaPink: "#F59E0B",
  starWhite: "#ffffff",
  textPrimary: "#F1F5F9",
  textSecondary: "#CBD5E1",
  textTertiary: "#94A3B8",
  hazardCritical: "#EF4444",
  hazardHigh: "#F97316",
  hazardMedium: "#EAB308",
  hazardLow: "#22C55E",
  gridLine: "rgba(255, 255, 255, 0.08)",
};

/**
 * Generate animated star field background
 */
function generateStars() {
  const starsContainer = document.getElementById("stars");
  if (!starsContainer) return;
  const starCount = 120;

  for (let i = 0; i < starCount; i++) {
    const star = document.createElement("div");
    star.className = "star";
    star.style.left = `${Math.random() * 100}%`;
    star.style.top = `${Math.random() * 100}%`;
    const size = Math.random() * 1.8 + 0.8;
    star.style.width = `${size}px`;
    star.style.height = `${size}px`;
    star.style.setProperty("--twinkle-duration", `${Math.random() * 4 + 3}s`);
    star.style.setProperty("--twinkle-opacity", `${Math.random() * 0.45 + 0.3}`);
    star.style.animationDelay = `${Math.random() * 3}s`;
    starsContainer.appendChild(star);
  }
}

document.addEventListener("DOMContentLoaded", () => {
  generateStars();
  initializeChart();

  if (window.lucide) {
    lucide.createIcons();
  }

  if (window.VELOCITY_DATA && window.VELOCITY_DATA.length > 0) {
    updateChart(window.VELOCITY_DATA);
  }

  checkHealth();
  setInterval(checkHealth, 30000);

  updateLastUpdated();
});

/**
 * Initialize Chart.js velocity chart with Sentinel Prime palette
 */
function initializeChart() {
  const canvas = document.getElementById("velocityChart");
  if (!canvas) return;
  const ctx = canvas.getContext("2d");

  // Teal-to-violet gradient fill
  const gradient = ctx.createLinearGradient(0, 0, 0, 400);
  gradient.addColorStop(0, "rgba(45, 212, 191, 0.5)");
  gradient.addColorStop(0.45, "rgba(56, 189, 248, 0.25)");
  gradient.addColorStop(1, "rgba(124, 58, 237, 0.0)");

  velocityChart = new Chart(ctx, {
    type: "line",
    data: {
      labels: [],
      datasets: [
        {
          label: "Velocity (km/h)",
          data: [],
          borderColor: colors.nebulaCyan,
          backgroundColor: gradient,
          borderWidth: 2.5,
          fill: true,
          tension: 0.4,
          pointRadius: 4,
          pointHoverRadius: 8,
          pointBackgroundColor: colors.nebulaCyan,
          pointBorderColor: "rgba(4, 13, 24, 0.8)",
          pointBorderWidth: 2,
          pointHoverBackgroundColor: colors.starWhite,
          pointHoverBorderColor: colors.nebulaCyan,
          pointHoverBorderWidth: 2.5,
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
          backgroundColor: "rgba(4, 13, 24, 0.97)",
          backdropFilter: "blur(24px)",
          titleColor: colors.textPrimary,
          bodyColor: colors.textSecondary,
          borderColor: "rgba(45, 212, 191, 0.25)",
          borderWidth: 1,
          padding: 14,
          displayColors: false,
          cornerRadius: 12,
          titleFont: {
            size: 13,
            weight: 700,
            family: "'Exo 2', sans-serif",
          },
          bodyFont: {
            size: 12,
            family: "'Nunito Sans', sans-serif",
          },
          callbacks: {
            title: (items) => velocityChart.data.labels[items[0].dataIndex],
            label: (item) => `Velocity: ${item.parsed.y.toLocaleString()} km/h`,
          },
        },
      },
      scales: {
        x: {
          grid: {
            color: colors.gridLine,
            drawBorder: false,
            lineWidth: 1,
          },
          ticks: {
            color: colors.textTertiary,
            maxRotation: 45,
            minRotation: 45,
            font: { size: 11, family: "'JetBrains Mono', monospace" },
            padding: 10,
          },
        },
        y: {
          grid: {
            color: colors.gridLine,
            drawBorder: false,
            lineWidth: 1,
          },
          ticks: {
            color: colors.textTertiary,
            font: { size: 11, family: "'JetBrains Mono', monospace" },
            padding: 10,
            callback: (value) => {
              if (value >= 1000000) return (value / 1000000).toFixed(1) + "M";
              if (value >= 1000) return (value / 1000).toFixed(0) + "K";
              return value.toLocaleString();
            },
          },
          beginAtZero: false,
        },
      },
    },
  });
}

/**
 * Update chart with grouped velocity data
 */
function updateChart(velocityData) {
  if (!velocityChart) return;

  if (!velocityData || velocityData.length === 0) {
    velocityChart.data.labels = ["No Data Available"];
    velocityChart.data.datasets[0].data = [0];
    velocityChart.options.scales.y.beginAtZero = true;
    velocityChart.update("none");
    return;
  }

  const groupedData = velocityData.reduce((acc, d) => {
    const date = new Date(d.date);
    const dateKey = date.toLocaleDateString("en-GB", {
      day: "2-digit",
      month: "short",
      year: "numeric",
    });

    if (!acc[dateKey]) {
      acc[dateKey] = { total: 0, count: 0, date: date };
    }
    acc[dateKey].total += d.velocity_km_per_h;
    acc[dateKey].count += 1;
    return acc;
  }, {});

  const sortedData = Object.entries(groupedData)
    .map(([dateKey, data]) => ({
      label: dateKey,
      velocity: data.total / data.count,
      date: data.date,
    }))
    .sort((a, b) => a.date - b.date);

  velocityChart.data.labels = sortedData.map((d) => d.label);
  velocityChart.data.datasets[0].data = sortedData.map((d) => d.velocity);
  velocityChart.options.scales.y.beginAtZero = false;
  velocityChart.update("none");
}

/**
 * Health check via API
 */
async function checkHealth() {
  try {
    const response = await fetch("/api/health");
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const result = await response.json();
    const healthy =
      result.success &&
      result.data &&
      result.data.status === "healthy" &&
      result.data.database_connected;
    updateConnectionStatus(healthy);
  } catch (error) {
    console.error("Health check failed:", error);
    updateConnectionStatus(false);
  }
}

/**
 * Update connection status badge
 */
function updateConnectionStatus(connected) {
  const statusDot = document.getElementById("connection-status");
  const statusText = document.getElementById("status-text");

  if (connected) {
    statusDot.className = "w-1.5 h-1.5 rounded-full status-dot status-dot-live";
    if (statusText) {
      statusText.textContent = "CONNECTED";
      statusText.className = "";
    }
  } else {
    statusDot.className = "w-1.5 h-1.5 rounded-full bg-hazard-critical";
    if (statusText) {
      statusText.textContent = "OFFLINE";
      statusText.className = "text-hazard-critical";
    }
  }
}

/**
 * Update last updated timestamp
 */
function updateLastUpdated() {
  const el = document.getElementById("last-updated");
  if (!el) return;
  const now = new Date();
  el.textContent = now.toLocaleTimeString("en-US", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function formatDate(dateString) {
  const date = new Date(dateString);
  return date.toLocaleDateString("en-US", {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

function formatDateTime(dateString) {
  const date = new Date(dateString);
  return date.toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function escapeHtml(text) {
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}

document.addEventListener("htmx:afterSwap", function (event) {
  if (window.lucide && window.lucide.createIcons) {
    window.lucide.createIcons();
  }

  if (event.detail.target.id === "velocity-chart-container") {
    const chartDataEl = document.getElementById("velocity-chart-data");
    if (chartDataEl) {
      const labelsAttr = chartDataEl.getAttribute("data-labels");
      const valuesAttr = chartDataEl.getAttribute("data-values");

      if (labelsAttr && valuesAttr) {
        try {
          const labels = JSON.parse(labelsAttr);
          const values = JSON.parse(valuesAttr);
          const velocityData = labels.map((label, index) => ({
            date: new Date(`2024-${label}`),
            velocity_km_per_h: values[index],
          }));
          updateChart(velocityData);
          updateLastUpdated();
        } catch (e) {
          console.error("Failed to parse velocity data:", e);
        }
      }
    }
  }
});

document.addEventListener("DOMContentLoaded", function () {
  if (window.lucide && window.lucide.createIcons) {
    window.lucide.createIcons();
  }
});

document.addEventListener("DOMContentLoaded", () => {
  const form = document.getElementById("filter-form");
  const startInput = document.getElementById("start-date");
  const endInput = document.getElementById("end-date");

  if (form && startInput && endInput) {
    startInput.addEventListener("change", () => {
      if (startInput.value) endInput.min = startInput.value;
    });

    endInput.addEventListener("change", () => {
      if (endInput.value) startInput.max = endInput.value;
    });

    form.addEventListener("htmx:beforeRequest", (e) => {
      if (
        startInput.value &&
        endInput.value &&
        endInput.value < startInput.value
      ) {
        alert("End date cannot be before start date.");
        e.preventDefault();
      }
    });
  }
});
