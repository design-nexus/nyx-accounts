# Provider setup and verification

This is an isolated development preview. Google and Microsoft have not been tested with real credentials. Do not merge based on mock tests.

## Google

1. Create a Google Cloud project that you control. Enable Gmail, Calendar, People and Drive APIs for the services you select (and Tasks API if selecting Tasks).
2. Configure Google Auth Platform branding, audience and data access. For a personal test project, choose External/Testing and add your Google account as a test user. Gmail IMAP access requires the restricted `https://mail.google.com/` scope. Publishing a broadly available application with restricted scopes can require verification; a personal test setup does not imply production approval.
3. Create an OAuth client of type **Desktop app**. Enter its client ID in Online Accounts. If the downloaded client configuration includes a client secret required by the token endpoint, enter it in the password field; it goes to Secret Service, never source control.
4. Select only services you intend to use, save, then choose Sign in. Authorization uses the system browser, PKCE S256 and a temporary IPv4 loopback listener. Refresh the account list after returning from the browser.
5. Google testing-mode refresh tokens can expire quickly (commonly seven days for scopes beyond basic identity). Reauthorize rather than treating this as a synchronization defect.
6. To revoke independently, remove the application's access in Google Account → Security → third-party connections. The daemon also implements Google's token revocation endpoint. Removing an account is a local disconnect and never deletes provider data.

References: https://developers.google.com/identity/protocols/oauth2/native-app ; https://developers.google.com/identity/protocols/oauth2/resources/loopback-migration ; https://developers.google.com/identity/protocols/oauth2#expiration

## Microsoft personal

1. Create your own application registration in Microsoft's application registration portal. Configure supported accounts as **personal Microsoft accounts only**.
2. Configure a public/native desktop redirect of exactly `http://127.0.0.1:42813/callback`. Microsoft documents that port matching is ignored only for `localhost`, so this provider uses a fixed loopback port. The portal text box may reject HTTP IP-literal redirects; use the application manifest/native redirect configuration as described in Microsoft’s redirect URI documentation. Do not register it as a confidential web application. If port 42813 is already occupied, sign-in fails instead of changing the redirect.
3. Configure delegated Graph permissions for selected services: User.Read; Mail.ReadWrite and Mail.Send; Calendars.ReadWrite; Contacts.ReadWrite; Files.ReadWrite; Tasks.ReadWrite when Tasks is selected. The flow requests offline_access and openid.
4. Enter the application/client ID. A public desktop application must not require a client secret. Sign in with Outlook.com, Hotmail.com, Live.com, or another personal Microsoft identity.
5. The service uses the `/consumers` authority exclusively. Work/school tenant administration, EWS and organizational Exchange are unsupported.
6. Revoke personal app permissions at https://account.live.com/consent/Manage . Graph's `revokeSignInSessions` is not supported for personal accounts; the service never claims otherwise.

References: https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow ; https://learn.microsoft.com/en-us/entra/identity-platform/reply-url ; https://learn.microsoft.com/en-us/graph/api/user-revokesigninsessions?view=graph-rest-1.0

## Fastmail

Create an app password granting the selected Mail/Calendars/Contacts access. Basic plans do not provide third-party protocol/app-password access. Presets use IMAP 993, SMTP 465 and Fastmail's DAV endpoints. DAV discovery finds collections before reading them. JMAP could reduce mail round trips and simplify incremental state, but replacing the existing IMAP engine is deferred.

References: https://www.fastmail.help/hc/en-us/articles/360058752854-App-passwords ; https://www.fastmail.com/dev/

## Zoho

Use an app password and select the actual account data centre. Confirm IMAP access is enabled and available on your plan; newly registered free plans may exclude it. Custom-domain paid accounts can use different IMAP/SMTP hosts from personal accounts. Choose the data centre in the region selector, then verify or override the populated endpoints using your account’s server settings.

Use the server names shown in your own Zoho account instead of inferring entitlement or endpoints from your email domain. Configure DAV endpoints explicitly: US documentation identifies `https://calendar.zoho.com/.well-known/caldav` and `https://contacts.zoho.com/.well-known/carddav`. Availability differs by service/account; do not assume regional substitution alone proves support. No Zoho OAuth flow is implemented; app passwords are the selected initial authentication path.

References: https://www.zoho.com/mail/help/imap-access.html ; https://www.zoho.com/calendar/help/setup-caldav-sync.html ; https://help.zoho.com/portal/en/kb/zoho-contacts/articles/carddav

## Nextcloud and generic protocols

Use an app password when supported. Enter secure IMAP (`imaps://`), SMTP (`smtps://` or `smtp+starttls://`), and HTTPS DAV endpoints. For Nextcloud copy the DAV URLs from your server's Calendar, Contacts and Files settings. Discovery remains on the configured origin; cross-origin redirects are refused to avoid leaking passwords. Generic-provider OAuth is not implemented without a supported provider registration/flow.

App passwords cannot be revoked remotely through a universal protocol. Delete them using the provider's security page. Local disconnect removes the service's keyring entry and account metadata, while preserving cached application data.

## Identity and preview storage

Enter the account's primary email for initial OAuth authorization. Later reauthentication is checked against its immutable provider identifier; connecting another person requires a separate profile, preserving existing cache associations. The isolated preview uses the existing desktop Secret Service with a separate keyring namespace. It does not provision a second keyring or copy GOA secrets.
