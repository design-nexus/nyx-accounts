#!/usr/bin/python3
"""Promotion regression: synthetic SQLite stores only; no bus or keyring access."""
import json
from pathlib import Path
import runpy
import sqlite3
import tempfile

promote = runpy.run_path(str(Path(__file__).resolve().parents[1] / 'packaging/nyx-accounts-promote'))['promote']
with tempfile.TemporaryDirectory(prefix='nyx-promote-fixture-') as temporary:
    root = Path(temporary)
    preview, data = root / 'preview', root / 'normal'
    accounts = preview / 'data/nyx-accounts-preview/accounts.sqlite3'
    pim = preview / 'data/nyx-pim-preview/pim.sqlite3'
    for path in (accounts, pim):
        path.parent.mkdir(parents=True)
    account = {'id': 'native-google', 'provider': 'google', 'identity': 'test@example.com', 'legacy_id': None, 'status': 'connected'}
    with sqlite3.connect(accounts) as db:
        db.execute('CREATE TABLE accounts(id TEXT PRIMARY KEY,metadata TEXT NOT NULL)')
        db.execute('INSERT INTO accounts VALUES(?,?)', (account['id'], json.dumps(account)))
    with sqlite3.connect(pim) as db:
        db.execute('CREATE TABLE objects(uid TEXT,payload TEXT,dirty INTEGER)')
        db.execute("INSERT INTO objects VALUES('one','pending edit',1)")
    local = data / 'nyx-mail/draft.txt'
    local.parent.mkdir(parents=True)
    local.write_text('original unsent draft')
    promote(preview, data, {'accounts': [{'provider': 'google', 'identity': 'TEST@example.com', 'legacy_id': 'old-goa'}]})
    with sqlite3.connect(data / 'nyx-accounts-preview/accounts.sqlite3') as db:
        copied = json.loads(db.execute('SELECT metadata FROM accounts').fetchone()[0])
        assert copied['id'] == 'native-google' and copied['status'] == 'connected' and copied['legacy_id'] == 'old-goa'
    with sqlite3.connect(accounts) as db:
        assert json.loads(db.execute('SELECT metadata FROM accounts').fetchone()[0])['legacy_id'] is None
    with sqlite3.connect(data / 'nyx-pim-preview/pim.sqlite3') as db:
        assert db.execute('SELECT payload,dirty FROM objects').fetchone() == ('pending edit', 1)
    assert local.read_text() == 'original unsent draft'
    assert (preview / 'PROMOTED').exists()
    try:
        promote(preview, data, {})
        raise AssertionError('Repeat promotion must refuse overwrite')
    except RuntimeError:
        pass
    print('PASS: promotion preserves native identity, credential reference, pending edits, originals and local drafts; repeat promotion refused')
