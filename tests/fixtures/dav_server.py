"""Loopback TLS DAV fixture. No provider accounts, user data or live credentials."""
import base64, http.server, ssl, sys
class Fixture(http.server.BaseHTTPRequestHandler):
    def log_message(self,*args): pass
    def authenticated(self):
        if self.headers.get('Authorization')!='Basic '+base64.b64encode(b'fixture@example.test:fixture-password').decode():
            self.send_error(401);return False
        return True
    def respond(self,body='',status=200,headers=None):
        self.send_response(status)
        self.send_header('Content-Length',str(len(body.encode())))
        for key,value in (headers or {}).items():self.send_header(key,value)
        self.end_headers();self.wfile.write(body.encode())
    def do_OPTIONS(self):
        if self.authenticated():self.respond(headers={'DAV':'1, calendar-access, calendar-auto-schedule'})
    def do_PROPFIND(self):
        if not self.authenticated():return
        self.rfile.read(int(self.headers.get('Content-Length',0)))
        props='<d:current-user-principal><d:href>/principals/me</d:href></d:current-user-principal>' if self.path=='/calendars' else '<c:schedule-outbox-URL><d:href>/outbox</d:href></c:schedule-outbox-URL><c:calendar-user-address-set><d:href>mailto:fixture@example.test</d:href></c:calendar-user-address-set>'
        self.respond('<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:response><d:propstat><d:prop>'+props+'</d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>',207)
    def do_POST(self):
        if not self.authenticated():return
        payload=self.rfile.read(int(self.headers.get('Content-Length',0))).decode()
        if self.path!='/outbox' or 'METHOD:REQUEST' not in payload or 'ATTENDEE:mailto:friend@example.test' not in payload or 'text/calendar' not in self.headers.get('Content-Type',''):
            self.send_error(400);return
        calendar='BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VFREEBUSY\r\nATTENDEE:mailto:friend@example.test\r\nFREEBUSY:20261030T120000Z/20261030T130000Z\r\nEND:VFREEBUSY\r\nEND:VCALENDAR\r\n'
        self.respond('<c:schedule-response xmlns:c="urn:ietf:params:xml:ns:caldav" xmlns:d="DAV:"><c:response><c:recipient><d:href>mailto:friend@example.test</d:href></c:recipient><c:request-status>2.0;Success</c:request-status><c:calendar-data><![CDATA['+calendar+']]></c:calendar-data></c:response></c:schedule-response>')
    def do_PUT(self):
        if not self.authenticated():return
        self.rfile.read(int(self.headers.get('Content-Length',0)))
        if 'text/vcard' not in self.headers.get('Content-Type',''):
            self.send_error(415);return
        if self.path=='/book/conflict.vcf':self.send_error(412);return
        if self.headers.get('If-None-Match')!='*' and self.headers.get('If-Match')!='"v1"':self.send_error(412);return
        self.respond(status=201,headers={'ETag':'"v2"' if self.headers.get('If-Match') else '"v1"'})
    def do_DELETE(self):
        if not self.authenticated():return
        if self.headers.get('If-Match')!='"v2"':self.send_error(412);return
        self.respond(status=204)
server=http.server.HTTPServer(('127.0.0.1',0),Fixture)
context=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER);context.load_cert_chain(sys.argv[1],sys.argv[2]);server.socket=context.wrap_socket(server.socket,server_side=True)
print(server.server_port,flush=True);server.serve_forever()
