"""A Live Client Data API of our own, for testing the League of Legends part
without the game: run it, start the pet with WAKUWAKU_LOL_URL=http://127.0.0.1:47290
(src-tauri/src/lol.rs reads from there, no game process looked for).

Its game clock runs from --at (seconds); the events, players and the one
playing are made from Riot's documented fields (developer.riotgames.com/docs/lol,
Game Client API), the events in the shape of Riot's sample
(src-tauri/fixtures/lol/events.json).

  python tools/lol_fake.py [--port 47290] [--at 280]

  POST /fake/event  {"EventName": "DragonKill", "DragonType": "Fire", ...}: at the clock now
  POST /fake/time   {"at": 600}: the clock to there
  POST /fake/items  {"riotId": "...", "items": [{"itemID": 3140, "displayName": "..."}]}
  POST /fake/me     {"isDead": true} etc.: the one playing's own fields
Standard library only.
"""
import argparse
import json
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

LOCK = threading.Lock()
STATE = {'start': time.time(), 'at': 0.0, 'events': [], 'next_id': 0}


def player(riot_id, team, position, champion, one, two, items=()):
    return {
        'riotId': riot_id, 'riotIdGameName': riot_id.split('#')[0], 'riotIdTagLine': riot_id.split('#')[1],
        'summonerName': riot_id, 'team': team, 'position': position, 'championName': champion, 'isBot': False,
        'isDead': False, 'respawnTimer': 0.0, 'level': 6, 'skinID': 0,
        'summonerSpells': {'summonerSpellOne': {'displayName': one}, 'summonerSpellTwo': {'displayName': two}},
        'items': [dict(i) for i in items],
        'scores': {'kills': 0, 'deaths': 0, 'assists': 0, 'creepScore': 0, 'wardScore': 0.0},
    }


PLAYERS = [
    player('Me#TW1', 'ORDER', 'MIDDLE', 'Ahri', 'Flash', 'Ignite'),
    player('Top#TW1', 'ORDER', 'TOP', 'Garen', 'Flash', 'Teleport'),
    player('Jg#TW1', 'ORDER', 'JUNGLE', 'Lee Sin', 'Flash', 'Smite'),
    player('Adc#TW1', 'ORDER', 'BOTTOM', 'Jinx', 'Flash', 'Heal'),
    player('Sup#TW1', 'ORDER', 'UTILITY', 'Lulu', 'Flash', 'Exhaust'),
    player('Foe1#KR1', 'CHAOS', 'TOP', 'Darius', 'Flash', 'Teleport'),
    player('Foe2#KR1', 'CHAOS', 'JUNGLE', 'Graves', 'Flash', 'Smite'),
    player('Foe3#KR1', 'CHAOS', 'MIDDLE', 'Zed', 'Flash', 'Ignite'),
    player('Foe4#KR1', 'CHAOS', 'BOTTOM', 'Caitlyn', 'Flash', 'Heal'),
    player('Foe5#KR1', 'CHAOS', 'UTILITY', 'Thresh', 'Flash', 'Ignite'),
]
ME = {'riotId': 'Me#TW1', 'summonerName': 'Me#TW1', 'currentGold': 1250.0, 'level': 6}


def clock():
    return STATE['at'] + (time.time() - STATE['start'])


class Handler(BaseHTTPRequestHandler):
    def send(self, body, status=200):
        data = json.dumps(body, ensure_ascii=False).encode('utf-8')
        self.send_response(status)
        self.send_header('Content-Type', 'application/json; charset=utf-8')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        path = self.path.split('?')[0]
        with LOCK:
            if path == '/liveclientdata/gamestats':
                return self.send({'gameMode': 'CLASSIC', 'gameTime': clock(), 'mapName': 'Map11', 'mapNumber': 11, 'mapTerrain': 'Default'})
            if path == '/liveclientdata/eventdata':
                return self.send({'Events': STATE['events']})
            if path == '/liveclientdata/playerlist':
                return self.send(PLAYERS)
            if path == '/liveclientdata/activeplayer':
                return self.send(ME)
        self.send({'error': 'not found'}, 404)

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers.get('Content-Length') or 0)) or b'{}')
        with LOCK:
            if self.path == '/fake/event':
                event = {'EventID': STATE['next_id'], 'EventTime': clock(), **body}
                STATE['next_id'] += 1
                STATE['events'].append(event)
                return self.send(event)
            if self.path == '/fake/time':
                STATE['at'], STATE['start'] = float(body['at']), time.time()
                return self.send({'at': clock()})
            if self.path == '/fake/items':
                for p in PLAYERS:
                    if p['riotId'] == body['riotId']:
                        p['items'] = body['items']
                return self.send({'ok': True})
            if self.path == '/fake/me':
                ME.update({k: v for k, v in body.items() if k in ME or k == 'currentGold'})
                for p in PLAYERS:
                    if p['riotId'] == ME['riotId']:
                        p.update({k: v for k, v in body.items() if k in p and k != 'scores'})
                        p['scores'].update(body.get('scores', {}))
                return self.send({'ok': True})
        self.send({'error': 'not found'}, 404)

    def log_message(self, *args):
        pass


def main():
    parser = argparse.ArgumentParser(description='A Live Client Data API of our own (League of Legends), for tests')
    parser.add_argument('--port', type=int, default=47290)
    parser.add_argument('--at', type=float, default=280.0, help='the game clock to start at (s)')
    args = parser.parse_args()
    STATE['at'], STATE['start'] = args.at, time.time()
    STATE['events'].append({'EventID': 0, 'EventName': 'GameStart', 'EventTime': 0.0})
    STATE['next_id'] = 1
    print(f'Live Client (fake) on http://127.0.0.1:{args.port}, the clock from {args.at:.0f} s')
    ThreadingHTTPServer(('127.0.0.1', args.port), Handler).serve_forever()


if __name__ == '__main__':
    main()
