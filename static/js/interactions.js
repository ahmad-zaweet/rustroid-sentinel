/**
 * Rustroid Sentinel - Interactive Effects
 * Spotlight, animations, and accessibility utilities
 * Aceternity-inspired + Apple Glassmorphism
 */

// ============================================
// Spotlight Hover Effect (Aceternity-inspired)
// ============================================
function initSpotlightEffects() {
    const spotlightCards = document.querySelectorAll('.spotlight-card, .stat-card, .chart-card');

    spotlightCards.forEach(card => {
        const spotlight = card.querySelector('.spotlight');
        if (!spotlight) return;

        card.addEventListener('mousemove', (e) => {
            const rect = card.getBoundingClientRect();
            const x = e.clientX - rect.left;
            const y = e.clientY - rect.top;

            // Update CSS custom properties for spotlight position
            spotlight.style.setProperty('--mouse-x', `${x}px`);
            spotlight.style.setProperty('--mouse-y', `${y}px`);

            // Create radial gradient following cursor
            spotlight.style.background = `radial-gradient(
                400px circle at ${x}px ${y}px,
                rgba(167, 139, 250, 0.12),
                transparent 40%
            )`;
        });

        card.addEventListener('mouseleave', () => {
            spotlight.style.opacity = '0';
        });

        card.addEventListener('mouseenter', () => {
            spotlight.style.opacity = '1';
        });
    });
}

// ============================================
// Staggered Entrance Animation
// ============================================
function animateEntrance() {
    const elements = document.querySelectorAll('.stat-card, .metric-card, .glass-card');

    const observer = new IntersectionObserver((entries) => {
        entries.forEach((entry, index) => {
            if (entry.isIntersecting) {
                const delayClass = `delay-${Math.min(index * 100, 500)}`;
                entry.target.classList.add('animate-fade-slide-up', delayClass);
                observer.unobserve(entry.target);
            }
        });
    }, {
        threshold: 0.1,
        rootMargin: '0px 0px -50px 0px'
    });

    elements.forEach(el => observer.observe(el));
}

// ============================================
// SSE Update Animation Trigger
// ============================================
function triggerUpdateAnimation(elementId) {
    const element = document.getElementById(elementId);
    if (!element) return;

    element.classList.add('animate-update-flash');
    setTimeout(() => {
        element.classList.remove('animate-update-flash');
    }, 600);
}

// ============================================
// Loading Skeleton Display
// ============================================
function showSkeleton(containerId, count = 3) {
    const container = document.getElementById(containerId);
    if (!container) return;

    container.innerHTML = Array(count).fill(`
        <div class="skeleton h-20 rounded-2xl mb-3"></div>
    `).join('');
}

// ============================================
// Reduced Motion Detection
// ============================================
function prefersReducedMotion() {
    return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
}

// ============================================
// Toast Notification System
// ============================================
function showToast(message, type = 'info') {
    const existing = document.getElementById('toast-notification');
    if (existing) existing.remove();

    const typeClasses = {
        info: 'bg-nebula-blue/90 border-nebula-blue',
        success: 'bg-hazard-low/90 border-hazard-low',
        error: 'bg-hazard-critical/90 border-hazard-critical',
        warning: 'bg-hazard-medium/90 border-hazard-medium'
    };

    const typeIcons = {
        info: 'ℹ️',
        success: '✅',
        error: '❌',
        warning: '⚠️'
    };

    const toast = document.createElement('div');
    toast.id = 'toast-notification';
    toast.className = `fixed bottom-6 right-6 ${typeClasses[type]} text-white px-6 py-4 rounded-2xl shadow-glass font-body font-medium text-sm z-50 backdrop-blur-glass border animate-fade-slide-up`;
    toast.innerHTML = `
        <div class="flex items-center gap-3">
            <span>${typeIcons[type]}</span>
            <span>${escapeHtml(message)}</span>
        </div>
    `;

    document.body.appendChild(toast);

    // Auto-remove after 5 seconds
    setTimeout(() => {
        toast.style.opacity = '0';
        toast.style.transform = 'translateY(16px)';
        setTimeout(() => toast.remove(), 300);
    }, 5000);
}

// ============================================
// Utility: Escape HTML
// ============================================
function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

// ============================================
// Initialize on DOM Ready
// ============================================
document.addEventListener('DOMContentLoaded', () => {
    initSpotlightEffects();
    animateEntrance();

    // Log reduced motion preference for debugging
    if (prefersReducedMotion()) {
        console.log('Reduced motion preference detected - animations disabled');
    }
});

// Export for use in other scripts
window.RustroidUI = {
    triggerUpdateAnimation,
    showSkeleton,
    showToast,
    prefersReducedMotion,
    escapeHtml
};
