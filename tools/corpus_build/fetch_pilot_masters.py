import os, sys, json, time, urllib.request, concurrent.futures
from pathlib import Path

STANDARD_ITEMS = [
    ('badpanda018', 'standard_01_riding_alone'),
    ('badpanda072', 'standard_02_badpanda072'),
    ('pn037', 'standard_03_boo_boo'),
    ('DWK217', 'standard_04_nightwalker'),
    ('VodorLZeckWelfare-disco', 'standard_05_welfare_disco'),
    ('MARAV003A', 'standard_06_maravines'),
    ('MARAV005A', 'standard_07_distelfink'),
    ('MARAV022A', 'standard_08_worms_eye'),
    ('MARAV017A', 'standard_09_sloan'),
    ('MARAV041A', 'standard_10_bradley'),
]

STRESS_ITEMS = [
    ('78_house-of-the-rising-sun_josh-white-and-his-guitar_gbia0001628b', 'stress_vinyl_01_house_rising_sun'),
    ('78_cuban-rhythms_hotel-nacional-orchestra-beltran_gbia0002479', 'stress_vinyl_02_cuban_rhythms'),
    ('78_mr-sandman_the-chordettes-archie-bleyer-pat-ballard-archie-ballard_gbia0017988b', 'stress_vinyl_03_mr_sandman'),
    ('78_hark-the-herald-angels-sing_warren-averel-william-ashley-tappin_gbia0072877b', 'stress_vinyl_04_hark_herald'),
    ('78_i-want-a-hippopotamus-for-christmas_vicki-dale-the-peter-pan-orchestra_gbia0000281a', 'stress_vinyl_05_hippopotamus'),
    ('exp038', 'stress_ambient_06_melt_lp'),
    ('slc36.chuzausen-mr_default', 'stress_ambient_07_chuzausen'),
    ('saw01x2', 'stress_ambient_08_paranoid'),
    ('DDW001', 'stress_acoustic_09_built_from_sticks'),
    ('unfound13', 'stress_acoustic_10_unhappy_anniversary'),
]

def get_flac_info(identifier):
    meta_url = f'https://archive.org/metadata/{identifier}/files'
    req = urllib.request.Request(meta_url, headers={'User-Agent': 'BDJStudio/1.0'})
    with urllib.request.urlopen(req, timeout=15) as resp:
        data = json.loads(resp.read().decode('utf-8'))
        files = data.get('result', [])
        flacs = [f for f in files if f.get('format') == 'Flac' or f.get('name', '').lower().endswith('.flac')]
        if not flacs:
            return None
        flacs.sort(key=lambda x: int(x.get('size', 0)))
        valid = [f for f in flacs if int(f.get('size', 0)) > 2 * 1024 * 1024]
        selected = valid[0] if valid else flacs[0]
        return selected['name'], int(selected.get('size', 0))

def download_file(url, out_path, expected_size):
    if out_path.exists() and out_path.stat().st_size > 1000:
        if expected_size and out_path.stat().st_size == expected_size:
            print(f'  [CACHE] {out_path.name} ({out_path.stat().st_size / 1024 / 1024:.1f} MB)')
            return True

    part_path = out_path.with_suffix('.flac.part')
    req = urllib.request.Request(url, headers={'User-Agent': 'BDJStudio/1.0'})
    t0 = time.time()
    downloaded = 0
    with urllib.request.urlopen(req, timeout=30) as resp, open(part_path, 'wb') as f_out:
        while True:
            chunk = resp.read(256 * 1024)
            if not chunk:
                break
            f_out.write(chunk)
            downloaded += len(chunk)
    
    os.replace(part_path, out_path)
    dt = time.time() - t0
    mb = downloaded / (1024 * 1024)
    speed = mb / dt if dt > 0 else 0
    print(f'  [OK] {out_path.name} ({mb:.1f} MB in {dt:.1f}s, {speed:.2f} MB/s)')
    return True

def fetch_item(item_id, target_stem):
    try:
        info = get_flac_info(item_id)
        if not info:
            print(f'  [WARN] No FLAC found for {item_id}')
            return False
        remote_name, size = info
        download_url = f'https://archive.org/download/{item_id}/{urllib.parse.quote(remote_name)}'
        out_path = Path('tools/corpus_test/pilot_masters') / f'{target_stem}.flac'
        return download_file(download_url, out_path, size)
    except Exception as e:
        print(f'  [ERROR] Failed {item_id}: {e}')
        return False

def main():
    out_dir = Path('tools/corpus_test/pilot_masters')
    out_dir.mkdir(parents=True, exist_ok=True)
    all_items = STANDARD_ITEMS + STRESS_ITEMS
    print(f'Iniciando descarga de {len(all_items)} másters certificados (10 estándar + 10 estrés)...')
    t_start = time.time()
    success_count = 0
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
        futures = {executor.submit(fetch_item, i, s): (i, s) for i, s in all_items}
        for future in concurrent.futures.as_completed(futures):
            if future.result():
                success_count += 1
    total_time = time.time() - t_start
    print(f'Completado: {success_count}/{len(all_items)} archivos en {total_time:.1f}s.')

if __name__ == '__main__':
    main()
