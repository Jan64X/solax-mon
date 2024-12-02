import requests
from flask import Flask, render_template, jsonify

app = Flask(__name__)

@app.route('/')
def dashboard():
    return render_template('dashboard.html')

@app.route('/data')
def data():
    # Fetch data from the external Solax endpoint
    try:
        response = requests.get('http://localhost:8000/')
        data = response.json()
        
        # Extract values for the dashboard
        return jsonify({
            "grid": data.get("Grid", "N/A"),
            "solar": data.get("Solar Panels", "N/A"),
            "batteries": float(data.get("Batteries", "0.0%").strip('%')) / 100,  # Convert to 0-1
            "batteriesText": data.get("Batteries", "N/A"),
            "homeConsumption": data.get("Home Consumption", "N/A")
        })
    except Exception as e:
        return jsonify({
            "error": str(e),
            "grid": "Error fetching data",
            "solar": "Error fetching data",
            "batteries": 0,
            "batteriesText": "N/A",
            "homeConsumption": "Error fetching data"
        })

if __name__ == '__main__':
    app.run(host='0.0.0.0', port=80)
