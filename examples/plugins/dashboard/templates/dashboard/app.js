// Dashboard example — client-side JavaScript.
// In a real plugin the refresh button would call the plugin RPC endpoint
// via the browsercli HTTP API.  Here we simulate data for demonstration.

(function () {
    "use strict";

    let requestCount = 0;
    let errorCount = 0;
    const startTime = Date.now();

    function randomInt(min, max) {
        return Math.floor(Math.random() * (max - min + 1)) + min;
    }

    function formatUptime(ms) {
        const s = Math.floor(ms / 1000);
        if (s < 60) return s + "s";
        const m = Math.floor(s / 60);
        if (m < 60) return m + "m " + (s % 60) + "s";
        const h = Math.floor(m / 60);
        return h + "h " + (m % 60) + "m";
    }

    function addLogEntry(message) {
        const ul = document.getElementById("event-log");
        const li = document.createElement("li");
        const ts = new Date().toLocaleTimeString();
        li.textContent = "[" + ts + "] " + message;
        ul.prepend(li);
        // Keep at most 50 entries.
        while (ul.children.length > 50) {
            ul.removeChild(ul.lastChild);
        }
    }

    function refresh() {
        // Simulate metric updates.
        requestCount += randomInt(1, 20);
        errorCount += Math.random() < 0.1 ? 1 : 0;
        const latency = randomInt(5, 120);
        const uptime = Date.now() - startTime;

        document.getElementById("metric-requests").textContent =
            requestCount.toLocaleString();
        document.getElementById("metric-errors").textContent =
            errorCount.toLocaleString();
        document.getElementById("metric-latency").textContent = latency + " ms";
        document.getElementById("metric-uptime").textContent =
            formatUptime(uptime);

        document.getElementById("last-updated").textContent =
            "Updated " + new Date().toLocaleTimeString();

        addLogEntry("Metrics refreshed — " + requestCount + " total requests");
    }

    // Initial refresh.
    refresh();

    // Auto-refresh every 3 seconds.
    setInterval(refresh, 3000);

    // Manual refresh button.
    document.getElementById("btn-refresh").addEventListener("click", function () {
        refresh();
        addLogEntry("Manual refresh triggered");
    });
})();
