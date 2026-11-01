#!/usr/bin/env python3
"""Synthetic migration fixture; original application stores and credentials are never opened."""
import hashlib,json,os,pathlib,sqlite3,subprocess,tempfile
root=pathlib.Path(__file__).resolve().parents[1]
binary=root.parent/'nyx-core/target/debug/nyx-pim-migrate'
with tempfile.TemporaryDirectory(prefix='nyx-migrate-fixture-') as temporary:
    base=pathlib.Path(temporary);(base/'original').mkdir();original=base/'original/attachment.txt';original.write_text('Original attachment remains untouched')
    calendar='BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTIMEZONE\r\nTZID:Fixture/Custom\r\nBEGIN:STANDARD\r\nDTSTART:19700101T000000\r\nTZOFFSETFROM:+0230\r\nTZOFFSETTO:+0230\r\nEND:STANDARD\r\nEND:VTIMEZONE\r\nBEGIN:VEVENT\r\nUID:series\r\nDTSTART;TZID=Fixture/Custom:20261030T090000\r\nRRULE:FREQ=DAILY;COUNT=3\r\nSUMMARY:Original series\r\nX-FIXTURE:preserve\r\nATTACH:'+original.as_uri()+'\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:series\r\nRECURRENCE-ID;TZID=Fixture/Custom:20261031T090000\r\nDTSTART;TZID=Fixture/Custom:20261031T100000\r\nSUMMARY:Exception\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n'
    bundle={'version':1,'accounts':[],'sources':[{'uid':'events','name':'Events','tasks':False,'color':'#123456','selected':False,'payloads':[calendar]},{'uid':'tasks','name':'Tasks','tasks':True,'color':None,'selected':True,'payloads':['BEGIN:VTODO\r\nUID:task\r\nSUMMARY:Task\r\nDUE;VALUE=DATE:20261031\r\nEND:VTODO\r\n']}],'books':[{'uid':'people','name':'People','cards':['BEGIN:VCARD\r\nVERSION:3.0\r\nUID:person\r\nFN:Original contact\r\nX-FIXTURE:preserve\r\nEND:VCARD\r\n']}],'warnings':[]}
    path=base/'export.json';path.write_text(json.dumps(bundle));digest=hashlib.sha256(path.read_bytes()).digest()
    env=dict(os.environ,HOME=str(base/'home'),XDG_DATA_HOME=str(base/'preview/data'),XDG_CONFIG_HOME=str(base/'preview/config'),XDG_CACHE_HOME=str(base/'preview/cache'))
    def run():subprocess.run([binary,'import',path],env=env,check=True,capture_output=True,text=True)
    run();dbpath=base/'preview/data/nyx-pim-preview/pim.sqlite3'
    with sqlite3.connect(dbpath) as db:
        assert db.execute("SELECT count(*) FROM objects WHERE source='import:events'").fetchone()[0]==2
        assert db.execute("SELECT count(*) FROM timezones WHERE source='import:events' AND tzid='Fixture/Custom'").fetchone()[0]==1
        payload=db.execute("SELECT payload FROM objects WHERE source='import:events' AND rid=''").fetchone()[0]
        assert original.as_uri() in payload and 'X-FIXTURE:preserve' in payload
        db.execute("UPDATE objects SET payload=replace(payload,'Original series','Edited locally'),dirty=1 WHERE source='import:events' AND rid=''")
        db.execute("UPDATE contacts SET payload=replace(payload,'Original contact','Edited contact'),dirty=1 WHERE book='import:people'")
        source=json.loads(db.execute("SELECT data FROM sources WHERE uid='import:events'").fetchone()[0]);source['name']='Renamed locally';source['selected']=True
        db.execute("UPDATE sources SET data=? WHERE uid='import:events'",(json.dumps(source),));db.commit()
    run()
    with sqlite3.connect(dbpath) as db:
        assert 'Edited locally' in db.execute("SELECT payload FROM objects WHERE source='import:events' AND rid=''").fetchone()[0]
        assert 'Edited contact' in db.execute("SELECT payload FROM contacts WHERE book='import:people'").fetchone()[0]
        assert json.loads(db.execute("SELECT data FROM sources WHERE uid='import:events'").fetchone()[0])['name']=='Renamed locally'
        assert db.execute("SELECT count(*) FROM objects WHERE source='import:events'").fetchone()[0]==2
    assert hashlib.sha256(path.read_bytes()).digest()==digest
    assert original.read_text()=='Original attachment remains untouched'
    print('PASS: isolated migration, recurrence/exception/timezone and raw fields, repeat-import edit protection, source preferences, untouched input and attachment')
