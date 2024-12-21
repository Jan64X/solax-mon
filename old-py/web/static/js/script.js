// Fetch data from Flask backend
async function fetchData() {
    try {
        const response = await fetch('/data');
        const data = await response.json();

        // Update Yield values
        document.getElementById('yield-value').textContent = data.yield;
        document.getElementById('system-to-home').textContent = data.systemToHome;

        // Update Consumption values
        document.getElementById('consumed-value').textContent = data.consumed;
        document.getElementById('from-grid').textContent = data.fromGrid;

        // Update circular progress bars
        updateProgress('.card:nth-child(1) .progress-circle', data.yield / data.yieldMax * 100);
        updateProgress('.card:nth-child(2) .progress-circle', data.consumed / data.consumedMax * 100);
    } catch (error) {
        console.error('Error fetching data:', error);
    }
}

// Update progress circle
function updateProgress(selector, percentage) {
    const circle = document.querySelector(selector);
    const radius = circle.r.baseVal.value;
    const circumference = 2 * Math.PI * radius;
    const offset = circumference - (percentage / 100) * circumference;
    circle.style.strokeDashoffset = offset;
}

// Refresh data every 5 seconds
setInterval(fetchData, 5000);

// Initial fetch
fetchData();
