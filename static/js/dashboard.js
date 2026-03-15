/**
 * Rustroid Sentinel Dashboard
 * Space-inspired Glassmorphic Design with Pagination & Timeline
 * Enhanced with Aceternity UI + Apple Glassmorphism
 *
 * Note: Pagination and filters now use HTMX for SSR.
 * Client-side state is kept for backward compatibility.
 */

// Global state
let velocityChart = null;
const REFRESH_INTERVAL_MS = 300000; // 5 minutes (not used - manual refresh only)

// Pagination state (deprecated - HTMX now handles pagination)
let currentPage = 1;
let totalPages = 1;
let pageSize = 20;
let totalItems = 0;

// Filter state (deprecated - HTMX now handles filters)
let currentFilters = {
  hazardClass: "",
  startDate: "",
  endDate: "",
};

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

  // Read SSR initialized state
  if (window.INITIAL_STATE) {
    currentPage = window.INITIAL_STATE.currentPage;
    totalPages = window.INITIAL_STATE.totalPages;
    pageSize = window.INITIAL_STATE.pageSize;
    totalItems = window.INITIAL_STATE.totalItems;
  }

  // Load velocity data from SSR (already embedded in page)
  if (window.VELOCITY_DATA && window.VELOCITY_DATA.length > 0) {
    updateChart(window.VELOCITY_DATA);
  }

  // ETL runs now loaded via HTMX
  loadMetrics();

  // Dynamic health checking
  checkHealth();
  setInterval(checkHealth, 30000); // Poll every 30 seconds

  updateTimelineButtons();
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
 * Load dashboard stats data
 */
async function loadDashboardData() {
  try {
    const response = await fetch("/api/stats");
    if (!response.ok) throw new Error(`HTTP ${response.status}`);

    const result = await response.json();

    if (result.success && result.data) {
      updateStats(result.data);
      updateLastUpdated();
    } else {
      console.error("API returned error:", result.error);
    }
  } catch (error) {
    console.error("Failed to load stats:", error);
    updateConnectionStatus(false);
  }
}

/**
 * Update stats cards with animation
 */
function updateStats(data) {
  animateValue("total-asteroids", data.total_asteroids);
  animateValue("total-approaches", data.total_approaches);
  animateValue("hazardous-count", data.hazardous_count);
}

/**
 * Animate number counting
 */
function animateValue(id, end) {
  const element = document.getElementById(id);
  const start = parseInt(element.textContent.replace(/,/g, "")) || 0;
  if (start === end) return;

  const duration = 800;
  const startTime = performance.now();

  function easeOutQuart(x) {
    return 1 - Math.pow(1 - x, 4);
  }

  function update(currentTime) {
    const elapsed = currentTime - startTime;
    const progress = Math.min(elapsed / duration, 1);
    const easedProgress = easeOutQuart(progress);
    const current = Math.floor(start + (end - start) * easedProgress);
    element.textContent = current.toLocaleString();
    if (progress < 1) requestAnimationFrame(update);
  }
  requestAnimationFrame(update);
}

/**
 * Load and display system health metrics
 */
async function loadMetrics() {
  try {
    const response = await fetch("/api/metrics/summary");
    if (!response.ok) return;

    const metrics = await response.json();

    // Update metrics display
    document.getElementById("metric-rps").textContent =
      metrics.requests_per_second.toFixed(3);

    // Error rate with color coding
    const errorRateEl = document.getElementById("metric-error-rate");
    errorRateEl.textContent = metrics.error_rate_percent.toFixed(3) + "%";
    if (metrics.error_rate_percent > 5) {
      errorRateEl.className =
        "text-2xl font-semibold mb-1 text-hazard-critical";
    } else if (metrics.error_rate_percent > 1) {
      errorRateEl.className = "text-2xl font-semibold mb-1 text-hazard-high";
    } else {
      errorRateEl.className = "text-2xl font-semibold mb-1 text-hazard-low";
    }

    // Latency with unit
    document.getElementById("metric-latency").textContent =
      metrics.avg_response_time_ms.toFixed(3) + " ms";

    // DB queries
    document.getElementById("metric-db-qps").textContent =
      metrics.db_queries_per_second.toFixed(3);
  } catch (error) {
    console.error("Failed to load metrics:", error);
    document.getElementById("metric-rps").textContent = "Err";
    document.getElementById("metric-error-rate").textContent = "Err";
    document.getElementById("metric-latency").textContent = "Err";
    document.getElementById("metric-db-qps").textContent = "Err";
    if (window.RustroidUI) {
      window.RustroidUI.showToast(
        "Telemetry connection failed. Metrics unavailable.",
        "error",
      );
    }
  }
}

/**
 * Helper to show toast notifications
 */
function showNotification(message) {
  const existing = document.getElementById("toast-notification");
  if (existing) existing.remove();

  const toast = document.createElement("div");
  toast.id = "toast-notification";
  toast.className =
    "fixed bottom-4 right-4 bg-hazard-critical text-white px-6 py-3 rounded-xl shadow-glass font-medium text-sm z-50 transition-smooth opacity-0 transform translate-y-4";
  toast.textContent = message;

  document.body.appendChild(toast);

  // Animate in
  setTimeout(() => {
    toast.classList.remove("opacity-0", "translate-y-4");
  }, 10);

  // Remove after 5 seconds
  setTimeout(() => {
    toast.classList.add("opacity-0", "translate-y-4");
    setTimeout(() => toast.remove(), 400);
  }, 5000);
}

/**
 * Set velocity timeline period
 */
function setVelocityPeriod(period) {
  currentVelocityPeriod = period;
  updateTimelineButtons();
  refreshVelocityData();
}

/**
 * Update timeline button active states
 */
function updateTimelineButtons() {
  document.querySelectorAll("[data-period]").forEach((btn) => {
    btn.classList.toggle(
      "active",
      btn.dataset.period === currentVelocityPeriod,
    );
  });
}

/**
 * Load velocity data with period filter (for manual refresh)
 */
async function refreshVelocityData() {
  try {
    const params = new URLSearchParams({ period: currentVelocityPeriod });
    const response = await fetch(`/api/velocity?${params}`);
    if (!response.ok) throw new Error(`HTTP ${response.status}`);

    const result = await response.json();

    if (result.success && result.data) {
      updateChart(result.data);
      // Update SSR data for consistency
      window.VELOCITY_DATA = result.data;
    }
  } catch (error) {
    console.error("Failed to load velocity data:", error);
  }
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
      velocity: data.total / data.count, // Average velocity for the day
      date: data.date,
    }))
    .sort((a, b) => a.date - b.date);

  velocityChart.data.labels = sortedData.map((d) => d.label);
  velocityChart.data.datasets[0].data = sortedData.map((d) => d.velocity);
  velocityChart.options.scales.y.beginAtZero = false;
  velocityChart.update("none");
}

/**
 * Load approaches with pagination and filters
 * NOTE: This function is now deprecated - pagination and filters use HTMX
 * Kept for backward compatibility and potential future use
 */
async function loadApproaches() {
  console.warn(
    "loadApproaches() is deprecated - use HTMX for pagination/filters",
  );
}

/**
 * Apply filters - DEPRECATED (HTMX now handles this)
 */
function applyFilters() {
  console.warn("applyFilters() is deprecated - HTMX handles filtering");
}

/**
 * Reset filters - DEPRECATED (HTMX now handles this)
 */
function resetFilters() {
  console.warn("resetFilters() is deprecated - HTMX handles filtering");
}

/**
 * Change page - DEPRECATED (HTMX now handles this)
 */
function changePage(page) {
  console.warn("changePage() is deprecated - HTMX handles pagination");
}

/**
 * Update pagination - DEPRECATED (HTMX now handles this)
 */
function updatePagination(pagination) {
  console.warn("updatePagination() is deprecated - HTMX handles pagination");
}

/**
 * Update the approaches table
 */
function updateApproachesTable(approaches) {
  const tbody = document.getElementById("approaches-body");

  if (!approaches || approaches.length === 0) {
    tbody.innerHTML = `
            <tr>
                <td colspan="5" class="px-8 py-16 text-center">
                    <div class="flex flex-col items-center gap-3">
                        <i
                            data-lucide="telescope"
                            class="w-12 h-12 text-text-muted"
                        ></i>
                        <p class="text-text-secondary font-medium">
                            No approach data available
                        </p>
                    </div>
                </td>
            </tr>
        `;
    return;
  }

  tbody.innerHTML = approaches
    .map(
      (approach) => `
        <tr class="table-row group border-b border-white/5 hover:bg-white/5 transition-all duration-300">
            <td class="px-8 py-5">
                <div class="flex items-center gap-3">
                    <div
                        class="w-8 h-8 rounded-lg bg-gradient-to-br from-nebula-purple/20 to-nebula-blue/20 backdrop-blur-sm border border-white/10 flex items-center justify-center"
                    >
                        <i
                            data-lucide="circle-dot"
                            class="w-4 h-4 text-nebula-purple"
                        ></i>
                    </div>
                    <span
                        class="font-heading font-semibold text-text-primary group-hover:text-nebula-purple transition-colors"
                    >${escapeHtml(approach.asteroid_name)}</span>
                </div>
            </td>
            <td class="px-8 py-5 text-text-secondary font-body">${formatDate(approach.close_approach_date)}</td>
            <td class="px-8 py-5 font-mono text-sm text-nebula-cyan font-bold">${formatVelocity(approach.velocity_km_per_h)}</td>
            <td class="px-8 py-5 font-mono text-sm text-nebula-blue font-bold">${formatDistance(approach.miss_distance_km)}</td>
            <td class="px-8 py-5">
                <span class="hazard-badge ${getHazardBadgeClass(approach.hazard_classification)}">
                    <span class="hazard-dot ${getHazardDotClass(approach.hazard_classification)}"></span>
                    ${escapeHtml(approach.hazard_classification)}
                </span>
            </td>
        </tr>
    `,
    )
    .join("");

  // Re-initialize Lucide icons for newly added elements
  if (window.lucide) {
    window.lucide.createIcons();
  }
}

/**
 * Format velocity with K suffix
 */
function formatVelocity(velocity) {
  return Math.floor(velocity / 1000) + "K";
}

/**
 * Format distance with M suffix
 */
function formatDistance(distance) {
  return (distance / 1000000).toFixed(2) + "M";
}

/**
 * Get hazard badge classes
 */
function getHazardBadgeClass(classification) {
  const classes = {
    Critical: "hazard-critical-badge",
    High: "hazard-high-badge",
    Medium: "hazard-medium-badge",
    Low: "hazard-low-badge",
  };
  return classes[classification] || classes["Low"];
}

/**
 * Get hazard dot classes
 */
function getHazardDotClass(classification) {
  const classes = {
    Critical: "hazard-critical-dot",
    High: "hazard-high-dot",
    Medium: "hazard-medium-dot",
    Low: "hazard-low-dot",
  };
  return classes[classification] || classes["Low"];
}

/**
 * Load ETL runs (HTMX handles this now, kept for manual refresh if needed)
 * @deprecated HTMX now handles ETL runs loading via /dashboard/etl-runs
 */
async function loadEtlRuns() {
  // HTMX handles loading via hx-get="/dashboard/etl-runs"
  // This function is kept for backward compatibility
  console.log("ETL runs are now loaded via HTMX");
}

/**
 * Update ETL runs display
 */
function updateEtlRuns(runs) {
  const container = document.getElementById("etl-runs");

  if (!runs || runs.length === 0) {
    container.innerHTML = `<div class="text-center text-text-secondary font-medium py-8">No ETL runs recorded</div>`;
    return;
  }

  container.innerHTML = runs
    .map(
      (run) => `
        <div class="p-4 glass-card rounded-2xl border border-white/10 hover:border-nebula-purple/40 transition-smooth" role="listitem">
            <div class="flex justify-between items-start mb-3">
                <div class="font-semibold text-text-primary text-sm truncate flex-1 pr-3" title="${escapeHtml(run.source_file)}">
                    ${escapeHtml(run.source_file)}
                </div>
                <span class="ml-2 px-2.5 py-1 rounded-full text-xs font-semibold backdrop-blur-sm ${getEtlStatusClass(run.status)}">
                    ${run.status}
                </span>
            </div>
            <div class="text-xs text-text-tertiary font-medium space-y-1.5">
                <div class="flex items-center gap-2">
                    <span class="w-1.5 h-1.5 rounded-full bg-nebula-purple"></span>
                    Started: ${formatDateTime(run.started_at)}
                </div>
                ${
                  run.completed_at
                    ? `
                <div class="flex items-center gap-2">
                    <span class="w-1.5 h-1.5 rounded-full bg-nebula-blue"></span>
                    Completed: ${formatDateTime(run.completed_at)}
                </div>`
                    : ""
                }
            </div>
            <div class="flex gap-4 mt-3 pt-3 border-t border-white/10">
                <div class="text-xs">
                    <span class="text-text-tertiary font-medium">Asteroids:</span>
                    <span class="text-text-secondary font-semibold ml-1">${run.asteroids_processed.toLocaleString()}</span>
                </div>
                <div class="text-xs">
                    <span class="text-text-tertiary font-medium">Approaches:</span>
                    <span class="text-text-secondary font-semibold ml-1">${run.approaches_processed.toLocaleString()}</span>
                </div>
            </div>
        </div>
    `,
    )
    .join("");
}

/**
 * Get ETL status badge classes
 */
function getEtlStatusClass(status) {
  const classes = {
    completed: "bg-hazard-low text-white border border-hazard-low/50",
    failed: "bg-hazard-critical text-white border border-hazard-critical/50",
    running: "bg-nebula-purple text-white border border-nebula-purple/50",
  };
  return (
    classes[status] || "bg-white/20 text-text-primary border border-white/20"
  );
}

/**
 * Show ETL error state
 */
function showEtlError(message) {
  const container = document.getElementById("etl-runs");
  container.innerHTML = `
        <div class="text-center py-8">
            <p class="text-text-secondary font-medium mb-4">${escapeHtml(message)}</p>
            <button hx-get="/dashboard/etl-runs?page=1" hx-target="#etl-runs" hx-swap="innerHTML" class="btn-glass px-6 py-2.5 rounded-xl text-sm font-semibold">Retry</button>
        </div>
    `;
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
 * HTMX 2.0 uses document events for compatibility
 */
document.addEventListener("htmx:afterSwap", function (event) {
  // Re-initialize Lucide icons for newly inserted content
  if (window.lucide && window.lucide.createIcons) {
    window.lucide.createIcons();
  }
});

// Initialize icons on initial page load
document.addEventListener("DOMContentLoaded", function () {
  if (window.lucide && window.lucide.createIcons) {
    window.lucide.createIcons();
  }
});

/**
 * Clear filter inputs when reset button is clicked
 */
document.addEventListener("htmx:beforeRequest", function (event) {
  // Check if the clicked element is the reset button
  if (event.detail.elt.getAttribute("aria-label") === "Reset filters") {
    // Clear the filter inputs
    const hazardFilter = document.getElementById("hazard-filter");
    const startDate = document.getElementById("start-date");
    const endDate = document.getElementById("end-date");

    if (hazardFilter) hazardFilter.value = "";
    if (startDate) startDate.value = "";
    if (endDate) endDate.value = "";
  }
});
