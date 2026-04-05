/**
 * Rustroid Sentinel Dashboard
 * Space-inspired Glassmorphic Design
 * Enhanced with Aceternity UI + Apple Glassmorphism
 *
 * Note: Pagination, filters, velocity chart, and metrics use HTMX for SSR.
 */

// Global state
let velocityChart = null;

// Velocity timeline state
let currentVelocityPeriod = "7d";

// Space color palette - High contrast for WCAG AA compliance
const colors = {
  nebulaPurple: "#a78bfa",
  nebulaBlue: "#60a5fa",
  nebulaCyan: "#22d3ee",
  nebulaPink: "#f472b6",
  starWhite: "#ffffff",
  textPrimary: "#ffffff",
  textSecondary: "#e5e7eb",
  textTertiary: "#9ca3af",
  hazardCritical: "#f87171",
  hazardHigh: "#fb923c",
  hazardMedium: "#facc15",
  hazardLow: "#4ade80",
  gridLine: "rgba(255, 255, 255, 0.15)",
};

/**
 * Generate animated star field background
 */
function generateStars() {
  const starsContainer = document.getElementById("stars");
  const starCount = 150;

  for (let i = 0; i < starCount; i++) {
    const star = document.createElement("div");
    star.className = "star";
    star.style.left = `${Math.random() * 100}%`;
    star.style.top = `${Math.random() * 100}%`;
    const size = Math.random() * 2 + 1;
    star.style.width = `${size}px`;
    star.style.height = `${size}px`;
    star.style.setProperty("--twinkle-duration", `${Math.random() * 3 + 2}s`);
    star.style.setProperty("--twinkle-opacity", `${Math.random() * 0.5 + 0.4}`);
    star.style.animationDelay = `${Math.random() * 2}s`;
    starsContainer.appendChild(star);
  }
}

/**
 * Initialize the dashboard on page load
 */
document.addEventListener("DOMContentLoaded", () => {
  generateStars();
  initializeChart();

  // Initialize Lucide icons
  if (window.lucide) {
    lucide.createIcons();
  }

  // Load velocity data from SSR (already embedded in page)
  if (window.VELOCITY_DATA && window.VELOCITY_DATA.length > 0) {
    updateChart(window.VELOCITY_DATA);
  }

  // Dynamic health checking
  checkHealth();
  setInterval(checkHealth, 30000); // Poll every 30 seconds

  updateLastUpdated();
});

/**
 * Initialize the Chart.js velocity chart
 */
function initializeChart() {
  const ctx = document.getElementById("velocityChart").getContext("2d");

  const gradient = ctx.createLinearGradient(0, 0, 0, 400);
  gradient.addColorStop(0, "rgba(167, 139, 250, 0.6)");
  gradient.addColorStop(0.5, "rgba(96, 165, 250, 0.4)");
  gradient.addColorStop(1, "rgba(34, 211, 238, 0.0)");

  velocityChart = new Chart(ctx, {
    type: "line",
    data: {
      labels: [],
      datasets: [
        {
          label: "Velocity (km/h)",
          data: [],
          borderColor: colors.nebulaBlue,
          backgroundColor: gradient,
          borderWidth: 3,
          fill: true,
          tension: 0.4,
          pointRadius: 5,
          pointHoverRadius: 9,
          pointBackgroundColor: colors.starWhite,
          pointBorderColor: colors.nebulaBlue,
          pointBorderWidth: 3,
          pointHoverBackgroundColor: colors.nebulaBlue,
          pointHoverBorderColor: colors.starWhite,
          pointHoverBorderWidth: 3,
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
          backgroundColor: "rgba(10, 10, 15, 0.98)",
          backdropFilter: "blur(24px)",
          titleColor: colors.starWhite,
          bodyColor: colors.textSecondary,
          borderColor: colors.nebulaPurple,
          borderWidth: 1,
          padding: 16,
          displayColors: false,
          cornerRadius: 16,
          titleFont: {
            size: 14,
            weight: 600,
            family: "'Rajdhani', sans-serif",
          },
          bodyFont: { size: 13, family: "'Titillium Web', sans-serif" },
          callbacks: {
            title: (items) => velocityChart.data.labels[items[0].dataIndex],
            label: (item) => `Velocity: ${item.parsed.y.toLocaleString()} km/h`,
          },
        },
      },
      scales: {
        x: {
          grid: { color: colors.gridLine, drawBorder: false, lineWidth: 1 },
          ticks: {
            color: colors.textSecondary,
            maxRotation: 45,
            minRotation: 45,
            font: { size: 12, family: "'Titillium Web', sans-serif" },
            padding: 12,
          },
        },
        y: {
          grid: { color: colors.gridLine, drawBorder: false, lineWidth: 1 },
          ticks: {
            color: colors.textSecondary,
            font: { size: 12, family: "'Titillium Web', sans-serif" },
            padding: 12,
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
 * Update the velocity chart with grouped data (by day/month/year)
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

  // Group velocity data by date (day/month/year) and calculate average velocity
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

  // Convert to array and sort by date
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
 * Check system health dynamically via API
 */
async function checkHealth() {
  try {
    const response = await fetch("/api/health");
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const result = await response.json();
    if (
      result.success &&
      result.data &&
      result.data.status === "healthy" &&
      result.data.database_connected
    ) {
      updateConnectionStatus(true);
    } else {
      updateConnectionStatus(false);
    }
  } catch (error) {
    console.error("Health check failed:", error);
    updateConnectionStatus(false);
  }
}

/**
 * Update connection status
 */
function updateConnectionStatus(connected) {
  const statusDot = document.getElementById("connection-status");
  const statusText = document.getElementById("status-text");

  if (connected) {
    statusDot.className = "w-2.5 h-2.5 rounded-full bg-hazard-low pulse-glow";
    statusText.textContent = "Connected";
    statusText.className = "text-sm font-semibold text-text-secondary";
  } else {
    statusDot.className = "w-2.5 h-2.5 rounded-full bg-hazard-critical";
    statusText.textContent = "Disconnected";
    statusText.className = "text-sm font-semibold text-text-secondary";
  }
}

/**
 * Update last updated timestamp
 */
function updateLastUpdated() {
  const now = new Date();
  document.getElementById("last-updated").textContent = now.toLocaleTimeString(
    "en-US",
    { hour: "2-digit", minute: "2-digit", second: "2-digit" },
  );
}

/**
 * Format date
 */
function formatDate(dateString) {
  const date = new Date(dateString);
  return date.toLocaleDateString("en-US", {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

/**
 * Format datetime
 */
function formatDateTime(dateString) {
  const date = new Date(dateString);
  return date.toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/**
 * Escape HTML
 */
function escapeHtml(text) {
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}

/**
 * Initialize Lucide icons after HTMX swaps content
 */
document.addEventListener("htmx:afterSwap", function (event) {
  // Re-initialize Lucide icons for newly inserted content
  if (window.lucide && window.lucide.createIcons) {
    window.lucide.createIcons();
  }

  // Handle velocity chart data updates
  if (event.detail.target.id === "velocity-chart-container") {
    const chartDataEl = document.getElementById("velocity-chart-data");
    if (chartDataEl) {
      const labelsAttr = chartDataEl.getAttribute("data-labels");
      const valuesAttr = chartDataEl.getAttribute("data-values");

      if (labelsAttr && valuesAttr) {
        try {
          const labels = JSON.parse(labelsAttr);
          const values = JSON.parse(valuesAttr);

          // Create velocity data points for updateChart
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

// Initialize icons on initial page load
document.addEventListener("DOMContentLoaded", function () {
  if (window.lucide && window.lucide.createIcons) {
    window.lucide.createIcons();
  }
});

/**
 * Filter Form Validation & Constraint Syncing
 */
document.addEventListener("DOMContentLoaded", () => {
  const form = document.getElementById("filter-form");
  const startInput = document.getElementById("start-date");
  const endInput = document.getElementById("end-date");

  if (form && startInput && endInput) {
    // 1. Sync min/max constraints dynamically
    startInput.addEventListener("change", () => {
      if (startInput.value) endInput.min = startInput.value;
    });

    endInput.addEventListener("change", () => {
      if (endInput.value) startInput.max = endInput.value;
    });

    // 2. Validate before HTMX request
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
