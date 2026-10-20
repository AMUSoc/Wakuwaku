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
  POST /fake/phase  {"phase": "ChampSelect"}: the client's (LCU) gameflow phase
  POST /fake/player {"riotId": "...", "position": "NONE", ...}: any of a player's fields
  POST /fake/gold   {"currentGold": 820}: the one playing's gold

It stands for the League client's own API too (WAKUWAKU_LCU_URL: gameflow
phase, current summoner, region, lobby, champion select: a duo in ranked
Solo/Duo, the other three hidden) and Riot's (WAKUWAKU_RIOT_API_URL:
league-v4 entries by PUUID, the key checked; for the game's ten, scout.rs:
spectator-v5, champion-mastery-v4, match-v5 ids and games, and the
client's champion names).
Standard library only.
"""
import argparse
import json
import threading
import time
import urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

LOCK = threading.Lock()
STATE = {'start': time.time(), 'at': 0.0, 'events': [], 'next_id': 0}


# Summoner spells as a Japanese client names them, with the game's own key
# (rawDescription) the names are found by whatever the language.
SPELLS = {
    'Flash': ('フラッシュ', 'SummonerFlash'), 'Teleport': ('テレポート', 'S12_SummonerTeleportUpgrade'),
    'Ignite': ('イグナイト', 'SummonerDot'), 'Smite': ('スマイト', 'SummonerSmite'), 'Heal': ('ヒール', 'SummonerHeal'),
    'Barrier': ('バリア', 'SummonerBarrier'), 'Exhaust': ('イグゾースト', 'SummonerExhaust'),
    'Cleanse': ('クレンズ', 'SummonerBoost'), 'Ghost': ('ゴースト', 'SummonerHaste'),
}


def spell(name):
    jp, key = SPELLS[name]
    return {'displayName': jp, 'rawDescription': f'GeneratedTip_SummonerSpell_{key}_Description', 'rawDisplayName': f'GeneratedTip_SummonerSpell_{key}_DisplayName'}


def player(riot_id, team, position, champion, one, two, items=()):
    return {
        'riotId': riot_id, 'riotIdGameName': riot_id.split('#')[0], 'riotIdTagLine': riot_id.split('#')[1],
        'summonerName': riot_id, 'team': team, 'position': position, 'championName': champion, 'isBot': False,
        'isDead': False, 'respawnTimer': 0.0, 'level': 6, 'skinID': 0,
        'summonerSpells': {'summonerSpellOne': spell(one), 'summonerSpellTwo': spell(two)},
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
CLIENT = {'phase': 'None'}
LOBBY = {'members': [{'puuid': 'client-puuid-me', 'gameName': 'Me', 'gameTag': 'TW1'}, {'puuid': 'client-puuid-duo', 'gameName': 'Duo', 'gameTag': 'TW1'}]}
SESSION = {'localPlayerCellId': 0, 'theirTeam': [{'cellId': 5, 'championId': 103}, {'cellId': 6, 'championId': 0}], 'myTeam': [
    {'cellId': 0, 'championId': 0, 'championPickIntent': 234, 'puuid': 'client-puuid-me', 'assignedPosition': 'middle'}, {'cellId': 1, 'championId': 0, 'puuid': 'client-puuid-duo', 'assignedPosition': 'utility'},
    {'puuid': '', 'assignedPosition': 'top'}, {'puuid': '', 'assignedPosition': 'jungle'}, {'puuid': '', 'assignedPosition': 'bottom'},
]}
RANKS = {
    'puuid-me': [{'queueType': 'RANKED_SOLO_5x5', 'tier': 'GOLD', 'rank': 'II', 'leaguePoints': 45, 'wins': 12, 'losses': 10}],
    'puuid-duo': [{'queueType': 'RANKED_FLEX_SR', 'tier': 'SILVER', 'rank': 'I', 'leaguePoints': 3, 'wins': 2, 'losses': 1}],
}
LOOKED_UP = []

# The game's ten as spectator-v5 has them, and a few games each.
CHAMPS = {103: '阿璃', 86: '蓋倫', 64: '李星', 222: '吉茵珂絲', 117: '露露', 122: '達瑞斯', 104: '格雷夫', 238: '劫', 51: '凱特琳', 412: '瑟雷西'}
TEN = [('puuid-me', 'Me#TW1', 100, 103), ('puuid-duo', 'Duo#TW1', 100, 117), ('puuid-top', 'Top#TW1', 100, 86),
       ('puuid-jg', 'Jg#TW1', 100, 64), ('puuid-adc', 'Adc#TW1', 100, 222), ('puuid-f1', 'Foe1#KR1', 200, 122),
       ('puuid-f2', 'Foe2#KR1', 200, 104), ('puuid-f3', 'Foe3#KR1', 200, 238), ('puuid-f4', 'Foe4#KR1', 200, 51),
       ('puuid-f5', 'Foe5#KR1', 200, 412)]
for i, (puuid, _, team, _) in enumerate(TEN):
    if puuid not in RANKS and i % 3:
        RANKS[puuid] = [{'queueType': 'RANKED_SOLO_5x5', 'tier': ['SILVER', 'GOLD', 'PLATINUM', 'EMERALD'][i % 4], 'rank': 'III', 'leaguePoints': 10 * i, 'wins': 20 + i, 'losses': 18}]


def match(puuid, champ, n):
    win = (n + len(puuid)) % 3 != 0
    return {'metadata': {'matchId': f'TW2_{puuid}_{n}'}, 'info': {'gameDuration': 1700, 'queueId': 420, 'gameEndTimestamp': 1000 - n, 'participants': [
        {'puuid': puuid, 'championId': champ if n % 2 == 0 else 1, 'win': win, 'kills': n % 7, 'deaths': 3, 'assists': 5}]}}


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
            if path.startswith('/lol-') or path.startswith('/riotclient'):
                if self.headers.get('Authorization') != 'Basic cmlvdDp0ZXN0':
                    return self.send({'error': 'unauthorized'}, 401)
                if path == '/lol-gameflow/v1/gameflow-phase':
                    return self.send(CLIENT['phase'])
                if path == '/lol-summoner/v1/current-summoner':
                    return self.send({'puuid': 'client-puuid-me', 'gameName': 'Me', 'tagLine': 'TW1'})
                if path == '/riotclient/region-locale':
                    return self.send({'region': 'TW2', 'locale': 'zh_TW'})
                if path == '/lol-lobby/v2/lobby':
                    return self.send(LOBBY)
                if path == '/lol-champ-select/v1/session' and CLIENT['phase'] == 'ChampSelect':
                    return self.send(SESSION)
            if path.startswith('/lol/league/v4/entries/by-puuid/'):
                if self.headers.get('X-Riot-Token') != 'RGAPI-test-key':
                    return self.send({'status': {'status_code': 403}}, 403)
                puuid = path.rsplit('/', 1)[1]
                LOOKED_UP.append(puuid)
                return self.send(RANKS.get(puuid, []))
            if path.startswith('/riot/account/v1/accounts/by-riot-id/'):
                # Riot's API knows a player by another PUUID than the client's: here 'puuid-' and their name.
                name = urllib.parse.unquote(path.split('/')[6])
                return self.send({'puuid': f'puuid-{name.lower()}', 'gameName': name, 'tagLine': path.split('/')[7]})
            if path.startswith('/lol/spectator/v5/active-games/by-summoner/'):
                if CLIENT['phase'] != 'InProgress':
                    return self.send({'status': {'status_code': 404}}, 404)
                return self.send({'gameId': 1, 'gameQueueConfigId': 420, 'participants': [
                    {'puuid': p, 'riotId': n, 'teamId': t, 'championId': c} for p, n, t, c in TEN]})
            if path.startswith('/lol/champion-mastery/v4/champion-masteries/by-puuid/') and 'by-champion' not in path and '/top' not in path:
                # Every champion's mastery, the most first (real ids: OP.GG is asked of them).
                pool = [234, 39, 92, 82, 875, 59, 157, 2, 11, 897, 64]
                return self.send([{'championId': c, 'championLevel': 40 - i * 3, 'championPoints': 2400000 // (i + 1)} for i, c in enumerate(pool)])
            if path.startswith('/lol/champion-mastery/v4/champion-masteries/by-puuid/'):
                puuid = path.split('/')[6]
                if puuid == 'puuid-adc':
                    return self.send({'status': {'status_code': 404}}, 404)
                return self.send({'championLevel': 7 + len(puuid) % 3, 'championPoints': 12345 * len(puuid)})
            if path.startswith('/lol/match/v5/matches/by-puuid/'):
                puuid = path.split('/')[6]
                return self.send([f'TW2_{puuid}_{n}' for n in range(10)])
            if path.startswith('/lol/match/v5/matches/'):
                _, puuid, n = path.rsplit('/', 1)[1].rsplit('_', 2)
                champ = next(c for p, _, _, c in TEN if p == puuid)
                return self.send(match(puuid, champ, int(n)))
            if path == '/lol-game-data/assets/v1/champion-summary.json':
                return self.send([{'id': k, 'name': v} for k, v in CHAMPS.items()])
            if path == '/fake/looked-up':
                return self.send(LOOKED_UP)
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
            if self.path == '/fake/player':
                for p in PLAYERS:
                    if p['riotId'] == body['riotId']:
                        p.update({k: v for k, v in body.items() if k != 'riotId'})
                return self.send({'ok': True})
            if self.path == '/fake/gold':
                ME['currentGold'] = float(body['currentGold'])
                return self.send(ME)
            if self.path == '/fake/select':
                # Her pick in champion select: {"championId": 234}, and theirs: {"enemies": [103, 122]}.
                if 'championId' in body:
                    SESSION['myTeam'][0]['championId'] = body['championId']
                if 'enemies' in body:
                    SESSION['theirTeam'] = [{'cellId': 5 + i, 'championId': c} for i, c in enumerate(body['enemies'])]
                return self.send(SESSION)
            if self.path == '/fake/phase':
                CLIENT['phase'] = body['phase']
                return self.send(CLIENT)
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
