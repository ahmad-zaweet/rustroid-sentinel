/**
 * Wires up a themed Flatpickr range picker on every form that has both a
 * `start_date` and `end_date` input (catalog filter, dashboard approach-log
 * filter). Re-runs after htmx swaps, since both host forms live inside
 * htmx-swapped containers and get replaced with fresh, uninitialized DOM.
 */
function initDateRangePickers(root) {
    if (typeof flatpickr === "undefined") return;

    root.querySelectorAll('input[name="start_date"]').forEach((start) => {
        if (start.dataset.flatpickrInit) return;

        const form = start.closest("form");
        const end = form && form.querySelector('input[name="end_date"]');
        if (!end) return;

        start.dataset.flatpickrInit = "true";
        end.dataset.flatpickrInit = "true";

        // rangePlugin drives `start` as a single range-mode instance under
        // the hood (mirroring its 2-date selection into the two visible
        // inputs), so picking a date earlier than the current start resets
        // the range from that date rather than producing end < start.
        //
        // The `change` dispatch happens on close, not on every date click:
        // both host forms swap themselves via htmx on `change`, and both
        // dates share one calendar, so firing after the first click would
        // reload the form (destroying the open calendar) before the second
        // click ever lands.
        flatpickr(start, {
            dateFormat: "Y-m-d",
            minDate: "2000-01-01",
            maxDate: "2100-12-31",
            defaultDate: start.value || undefined,
            plugins: [new rangePlugin({ input: end })],
            onClose: () => {
                start.dispatchEvent(new Event("change", { bubbles: true }));
            },
        });
    });
}

document.addEventListener("DOMContentLoaded", () => {
    initDateRangePickers(document);
});

document.addEventListener("htmx:afterSwap", (event) => {
    initDateRangePickers(event.detail.target);
});
