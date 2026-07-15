import json
from http.server import HTTPServer, BaseHTTPRequestHandler
import sys

# Mock state
state = {
    "mode": "auto",
    "current_angle": 0,
    "backend": "ESP32 S3 (Mocked)"
}

# 36 species mock list
species_list = [
    {"class_id": i, "name": f"Mosquito Species {i}", "slot": chr(65 + i) if i < 26 else f"{chr(65 + (i-26))}0", "angle": i * 10, "rgb": (i*7 % 256, i*13 % 256, i*17 % 256)}
    for i in range(36)
]
# Replace names with some real labels for realism
labels = [
    "Aedes camptorhynchus", "Aedes hesperonotius", "Aedes notoscriptus", "Aedes ratcliffei", "Aedes vigilax",
    "Anopheles annulipes", "Coquillettidia linealis", "Culex annulirostris", "Culex australicus", "Culex globocoxitus",
    "Culex latus", "Culex quinquefasciatus", "Aedes alboannulatus", "Aedes alternans", "Aedes sagax",
    "Coquillettidia xanthogaster", "Culex sitiens", "Male coquillettidia xanthogaster", "Mansonia uniformis",
    "Tripteroides atripes", "Aedes aculeatus", "Aedes alboseutellatus", "Aedes bitaenorhynchus", "Aedes burpengaryensis",
    "Aedes garnicola", "Aedes lineatopennis", "Aedes procox", "Aedes vittiger", "Anopheles bancroftii",
    "Culex arbostensis", "Verralina funerea", "Verrallina masters 52", "Culiseta atra", "Aedes albosostatus",
    "Aedes hesperontus", "Aedes_torneri"
]
for i, name in enumerate(labels):
    species_list[i]["name"] = name

class MockBackendHandler(BaseHTTPRequestHandler):
    def end_headers(self):
        self.send_header('Access-Control-Allow-Origin', '*')
        self.send_header('Access-Control-Allow-Methods', 'GET, POST, OPTIONS')
        self.send_header('Access-Control-Allow-Headers', 'Content-Type')
        super().end_headers()

    def do_OPTIONS(self):
        self.send_response(200)
        self.end_headers()

    def do_GET(self):
        if self.path == '/api/status':
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.end_headers()
            self.wfile.write(json.dumps(state).encode('utf-8'))
        elif self.path == '/api/species_list':
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.end_headers()
            self.wfile.write(json.dumps(species_list).encode('utf-8'))
        else:
            self.send_response(404)
            self.end_headers()

    def do_POST(self):
        content_length = int(self.headers.get('Content-Length', 0))
        post_data = self.rfile.read(content_length) if content_length > 0 else b''
        payload = json.loads(post_data.decode('utf-8')) if post_data else {}

        print(f"POST {self.path} with payload: {payload}")

        if self.path == '/api/mode/simulation':
            state["mode"] = "simulation"
            self.send_response(200)
        elif self.path == '/api/mode/auto':
            state["mode"] = "auto"
            self.send_response(200)
        elif self.path == '/api/reset_home':
            state["current_angle"] = 0
            self.send_response(200)
        elif self.path == '/api/rotate_left':
            deg = payload.get("degrees", 0)
            state["current_angle"] = (state["current_angle"] - deg + 360) % 360
            self.send_response(200)
        elif self.path == '/api/rotate_right':
            deg = payload.get("degrees", 0)
            state["current_angle"] = (state["current_angle"] + deg) % 360
            self.send_response(200)
        elif self.path == '/api/rotate_slot_to_angle':
            # Mock angle update
            state["current_angle"] = payload.get("angle", 0)
            self.send_response(200)
        elif self.path == '/api/move_slot_to_slot':
            self.send_response(200)
        elif self.path == '/api/simulate_mosquito':
            class_id = payload.get("class_id", 0)
            state["current_angle"] = class_id * 10
            self.send_response(200)
        else:
            self.send_response(404)
            self.end_headers()
            return
        
        self.end_headers()
        self.wfile.write(b'{"success":true}')

def run_server():
    server = HTTPServer(('127.0.0.1', 3000), MockBackendHandler)
    print("Mock Backend running on http://127.0.0.1:3000")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass

if __name__ == '__main__':
    run_server()
