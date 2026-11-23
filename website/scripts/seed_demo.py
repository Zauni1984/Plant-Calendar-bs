"""Demo account with realistic data for the website screenshots.

Runs against a LOCAL backend only (default http://127.0.0.1:8080):
    python scripts/seed_demo.py de demo-de@example.test <password>
    python scripts/seed_demo.py en demo-en@example.test <password>
Prints nothing secret; the password is the caller's own test value.
"""
import json
import sys
import urllib.error
import urllib.request
from datetime import datetime, timedelta, timezone

API = "http://127.0.0.1:8080"
LANG, EMAIL, PASSWORD = sys.argv[1], sys.argv[2], sys.argv[3]
token = None


def call(method, path, body=None):
    req = urllib.request.Request(API + path, data=json.dumps(body).encode() if body is not None else None, method=method)
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", "Bearer " + token)
    try:
        with urllib.request.urlopen(req) as r:
            raw = r.read()
            return r.status, json.loads(raw) if raw else None
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode(errors="replace")


st, res = call("POST", "/auth/register", {"email": EMAIL, "password": PASSWORD})
if st != 200:
    st, res = call("POST", "/auth/login", {"email": EMAIL, "password": PASSWORD})
token = res["access_token"]
for c in ("cloud_sync", "weather", "devices", "billing_stripe"):
    call("POST", "/me/consents", {"consent_type": c, "granted": True, "policy_version": "1.0"})

# Start from a clean account on every run.
st, plants = call("GET", "/plants")
for p in plants or []:
    call("DELETE", f"/plants/{p['plant_id']}")

de = LANG == "de"
now = datetime.now(timezone.utc).replace(hour=9, minute=0, second=0, microsecond=0)
iso = lambda d: d.isoformat().replace("+00:00", "Z")

st, beet = call("POST", "/locations", {"name": "Hochbeet" if de else "Raised bed", "location_type": "outdoor", "latitude": 49.03, "longitude": 11.47})
st, balcony = call("POST", "/locations", {"name": "Balkon" if de else "Balcony", "location_type": "balcony", "latitude": 49.03, "longitude": 11.47})
st, garden = call("POST", "/locations", {"name": "Obstgarten" if de else "Orchard", "location_type": "outdoor", "latitude": 49.03, "longitude": 11.47})

def plant(name, color, phase, loc, catalog, lifecycle="annual", week=3, pot=None, soil=None):
    st, p = call("POST", "/plants", {
        "name": name, "color_tag": color, "phase": phase, "phase_week": week, "location_id": loc["location_id"],
        "lifecycle": lifecycle, "catalog_id": catalog, "pot_size_liters": pot, "soil_type": soil,
    })
    assert st in (200, 201), (st, p)
    return p

tomato = plant("Tomate 'San Marzano'" if de else "Tomato 'San Marzano'", "#E53935", "flower", beet, "tomato", week=6)
basil = plant("Basilikum" if de else "Basil", "#2E7D32", "veg", balcony, "basil", week=4, pot=3, soil="erde")
pepper = plant("Paprika 'California Wonder'" if de else "Pepper 'California Wonder'", "#FF9800", "veg", beet, "pepper", week=5)
apple = plant("Apfel 'Elstar'" if de else "Apple 'Elstar'", "#C0392B", "veg", garden, "apple_elstar", lifecycle="perennial", week=1)
cherry = plant("Süßkirsche 'Kordia'" if de else "Sweet cherry 'Kordia'", "#922B21", "veg", garden, "cherry_kordia", lifecycle="perennial", week=1)

def task(p, title, cat, days, repeat=None, feeds=None):
    body = {"title": title, "category": cat, "due_at": iso(now + timedelta(days=days)), "repeat_interval_hours": repeat, "notification_enabled": True}
    if feeds:
        body["feeds"] = feeds
    st, t = call("POST", f"/plants/{p['plant_id']}/tasks", body)
    assert st in (200, 201), (st, t)

veggie = [{"brand": "Hanfjack", "product": "Veggie", "amount": 2, "unit": "ml/l"}]
bloom = [{"brand": "Hanfjack", "product": "Blüte", "amount": 3, "unit": "ml/l"}]
task(tomato, "Gießen" if de else "Water", "water", 0, 48)
task(basil, "Gießen" if de else "Water", "water", 1, 72)
task(pepper, "Gießen" if de else "Water", "water", 2, 48)
task(tomato, "Düngen (Blüte)" if de else "Feed (bloom)", "feed", 3, 168, bloom)
task(pepper, "Düngen (Wachstum)" if de else "Feed (growth)", "feed", 5, 168, veggie)
task(tomato, "Ausgeizen" if de else "Remove side shoots", "train", 4, 168)
task(apple, "Fruchtbehang prüfen" if de else "Check fruit set", "check", 9)
task(cherry, "Gegen Vögel einnetzen" if de else "Net against birds", "check", 12)
task(basil, "Blütenansätze entfernen" if de else "Pinch off flowers", "train", 6, 336)

def log(p, ltype, days_ago, text=None, **metrics):
    body = {"log_type": ltype, "created_at": iso(now - timedelta(days=days_ago, hours=-2)), "text": text}
    if metrics:
        body["metrics"] = metrics
    st, l = call("POST", f"/plants/{p['plant_id']}/logs", body)
    assert st in (200, 201), (st, l)

log(tomato, "feeding", 2, "Erste Blüten offen" if de else "First flowers open", feeds=bloom, water_liters=2.0, ph=6.4, ec=1.6)
log(pepper, "feeding", 4, None, feeds=veggie, water_liters=1.5, ph=6.3)
log(tomato, "measurement", 1, None, temp_c=24.5, rh=58, vpd=1.2, ppfd=650, soil_moisture=42)
log(basil, "watering", 1, None, water_liters=0.5, water_method="giessen")
log(tomato, "watering", 4, None, water_liters=2.5, water_method="tropf")
log(basil, "harvest", 6, "Für Pesto" if de else "For pesto", harvest_kg=0.15, harvest_quality="excellent")
for p in (tomato, basil, pepper, apple, cherry):
    log(p, "watering", 0, None, water_liters=1.0, water_method="giessen")
log(apple, "note", 10, "Blüte gut bestäubt, Bienen aktiv." if de else "Good pollination, lots of bees.")
print("seeded", LANG)
