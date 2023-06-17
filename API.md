# API

Das folgende Dokument beschreibt die Übersicht über die 
API des `dgb-server`.

### Authentifizierung

Um sich beim Server anzumelden, benötigt man ein Token, was
im HTTP-Header `Authentication` gesendet wird. Um dieses Token
zu generieren, muss eine Form-Anfrage mit `email` und `passwort`
nach `/login` gePOSTet werden:

#### Authentifizierung: Ok

- `status`: String: immer `"error"`
- `token`: String: Token, 30 Minuten lang gültig

#### Authentifizierung: Fehler

- `status`: String: immer `"error"`
- `code`: Integer: Fehlercode
    - 0: Benutzer nicht gefunden
    - 500: Interner Fehler
- `text`: String: Fehlermeldung vorformatiert als Text

#### Beispiel einer Such-Anfrage

```
POST /login HTTP/1.1
Content-Type: application/x-www-form-urlencoded
email=max@mustermann.de&passwort=abc123
```
```
{
    "status": "ok",
    "token": "S0VLU0UhIExFQ0tFUiEK"
}
```
```
GET /suche/Mein%20Suchbegriff HTTP/1.1
Content-Type: application/json
Authorization: Bearer S0VLU0UhIExFQ0tFUiEK
```
```
{
    "status": "ok",
    "grundbuecher": [],
    "aenderungen": []
}
```

### API-Übersicht

- Suche: `GET /suche/{suchbegriff}`: Durchsucht die Grundbuchblätter und 
  Änderungsmitteilungen nach `suchbegriff`
- Download:
    - `GET /download/gbx/{amtsgericht}/{grundbuch_von}/{blatt}`: 
      gibt die .gbx (= JSON) Datei des Grundbuchblatts von `amtsgericht`, 
      `grundbuch_von`, `blatt` als JSON aus
    - `GET /download/pdf/{amtsgericht}/{grundbuch_von}/{blatt}`: 
      gibt das Grundbuchblatt als PDF aus oder eine JSON-Fehlermeldung
- Upload:
    - `POST /upload`: Lädt eine Grundbuchänderung hoch (wenn Benutzerkonto + Signatur stimmen)
- Abonnements: Bei einer Änderung des abonnierten Grundbuchblatts wird der 
  entsprechede Webhook aktiviert bzw. eine E-Mail gesendet
    - `POST /abo-neu/email/{amtsgericht}/{grundbuchbezirk}/{blatt}`
    - `POST /abo-neu/webhook/{amtsgericht}/{grundbuchbezirk}/{blatt}`
    - `POST /abo-loeschen/{id}`

### Suche

Durchsucht Grundbuchblätter nach einem Suchbegriff

URL: GET `/suche`

```
GET https://127.0.0.1/suche/Suchbegriff HTTP/1.1
Authorization: Bearer S0VLU0UhIExFQ0tFUiEK
```

#### Suchergebnis: Ok

- `status`: String: immer `"ok"`
- `grundbuecher`: Array[Objekt]: Grundbücher, die den 
   Suchbegriff enthalten (max. 50 Ergebnisse)
    - `titelblatt`: Objekt: Titelblatt des gefundenen Grundbuchs
        - `amtsgericht`: String: Amtsgericht des gefundenen Grundbuchs
        - `grundbuch_von`: String: Grundbuchblattbezirk des gefundenen Grundbuchs
        - `blatt`: String: Blatt-Nr. des gefundenen Grundbuchs
    - `ergebnis`: Objekt: Suchergebnis im Suchindex
        - `land`: String: Bundesland des gefundenen Grundbuchs
        - `amtsgericht`: String: Amtsgericht des gefundenen Grundbuchs
        - `grundbuch_von`: String: Grundbuchblattbezirk des gefundenen Grundbuchs
        - `blatt`: String: Blatt-Nr. des gefundenen Grundbuchs
        - `abteilung`: String: Abteilung, in der der Suchbegriff gefunden wurde
            - `bv`: Bestandsverzeichnis
            - `bv-herrschvermerke`: Bestandsverzeichnis, aber der gefundene Eintrag ist ein HVM
            - `bv-zuschreibungen`: Bestandsverzeichnis (Zuschreibungen)
            - `bv-abschreibungen`: Bestandsverzeichnis (Abschreibungen)
            - `abt1`: Abteilung 1, Spalte 1 - 2
            - `abt1-grundlagen-eintragungen`: Abteilung 1, Spalte 3 - 4
            - `abt1-veraenderungen`: Abteilung 1 (Veränderungen)
            - `abt1-loeschungen`: Abteilung 1 (Löschungen)
            - `abt2`: Abteilung 2
            - `abt2-veraenderungen`: Abteilung 2 (Veränderungen)
            - `abt2-loeschungen`: Abteilung 2 (Löschungen)
            - `abt3`: Abteilung 3
            - `abt3-veraenderungen`: Abteilung 3 (Veränderungen)
            - `abt3-loeschungen`: Abteilung 3 (Löschungen)
        - `lfd_nr`: String: Laufende Nummer des gefundenen Texts
        - `text`: String: Gefundener Text
    - `abos`: Array[Objekt]: Abonnements für dieses Grundbuchblatt 
      (ein Blatt kann unter mehreren Aktenzeichen abonniert sein)
        - `amtsgericht`: String: Amtsgericht des Abonnements
        - `grundbuchbezirk`: String: Grundbuchbezirk des Abonnements
        - `blatt`: String: Blatt-Nr. des Abonnements
        - `text`: String: E-Mail des Abonnenten (`= "max@mustermann.de"`)
        - `aktenzeichen`: String: Aktenzeichen des Abonnements

#### Suchergebnis: Fehler

- `status`: String: immer `"error"`
- `code`: Integer: Fehlercode
    - 0: Benutzer nicht gefunden
    - 500: Interner Fehler
- `text`: String: Fehlermeldung vorformatiert als Text

### Download (PDF)

URL: GET `/download/pdf/{amtsgericht}/{grundbuch_von}/{blatt}`

Rendert den momentanen Stand des Grundbuchs in eine PDF-Datei

```
GET https://127.0.0.1/download/pdf/Prenzlau/Schenkenberg/289 HTTP/1.1
Authorization: Bearer S0VLU0UhIExFQ0tFUiEK
```

#### Download (PDF): Ok

PDF-Datei im Format `application/pdf`

#### Download (PDF): Fehler

JSON-Objekt mit den Feldern:

- `status`: String: immer `"error"`
- `code`: Integer: Fehlercode
    - 0: Benutzer nicht gefunden
    - 1: Ungültiges Amtsgericht / ungültiger Gemarkungsbezirk
    - 404: Grundbuchblatt existiert (noch) nicht
    - 500: Interner Fehler
- `text`: String: Fehlermeldung vorformatiert als Text

### Download (GBX)

URL: GET `/download/gbx/{amtsgericht}/{grundbuch_von}/{blatt}`

Gibt den momentanen Stand des Grundbuchs im JSON-Format (.gbx) zurück

```
GET https://127.0.0.1/download/gbx/Prenzlau/Schenkenberg/289 HTTP/1.1
Authorization: Bearer S0VLU0UhIExFQ0tFUiEK
```

#### Download (GBX): Ok

- `status`: String: immer `"ok"`
- `datei`, `gbx_datei_pfad`, `titelblatt`, ...: GBX-Datei, Format siehe unten

#### Download (GBX): Fehler

- `status`: String: immer `"error"`
- `code`: Integer: Fehlercode
    - 0: Benutzer nicht gefunden
    - 1: Ungültiges Amtsgericht / ungültiger Gemarkungsbezirk
    - 404: Grundbuchblatt existiert (noch) nicht
    - 500: Interner Fehler
- `text`: String: Fehlermeldung vorformatiert als Text

### EMail-Abonnement neu anlegen

Legt ein neues E-Mail-Abonnement für den Benutzer an (`aktenzeichen` ist optional)

URL: POST `/abo-neu/{typ}/{amtsgericht}/{grundbuchbezirk}/{blatt}`

FORM: 

    - `typ`: Typ des Abonnements, `email` oder `webhook`
    - `aktenzeichen`: Optional[String]: Aktenzeichen, was auf Benachrichtigungen bei 
      Grundbuchänderungen an diesem Blatt später bei "Ihr Zeichen" / "Unser Zeichen" 
      auftauchen wird.

```
POST https://127.0.0.1/abo-neu/email/Prenzlau/Schenkenberg/289 HTTP/1.1
Authorization: Bearer S0VLU0UhIExFQ0tFUiEK
Content-Type: application/x-www-form-urlencoded
aktenzeichen=ABC%20DEF
```

Wenn das Grundbuchblatt jetzt geändert wird, wird der Benutzer, welcher die Anfrage
gestellt hat, eine E-Mail mit Hinweis auf die Änderung erhalten, mit dem Hinweis auf
das Aktenzeichen "ABC DEF".

#### Abonnement neu anlegen: Ok

- `status`: String: immer `"ok"`

#### Abonnement neu anlegen: Fehler

- `status`: String: immer `"error"`
- `code`: Integer: Fehlercode
    - 0: Benutzer nicht gefunden
    - 500: Interner Fehler
- `text`: String: Fehlermeldung vorformatiert als Text

### Abonnement löschen

Funktioniert genau wie "EMail-Abonnement anlegen", siehe oben

URL: POST `/abo-loeschen/{id}`

### Webhook neu anlegen

Legt ein neues Webhook-Abonnement für den Benutzer an: In diesem Fall wird bei
einer Änderung "https://meinwebhook.com:8080" benachrichtigt.

URL: POST `/abo-neu/webhook/{amtsgericht}/{grundbuchbezirk}/{blatt}`

FORM: 

    - `url`: String: Server-URL, welche bei einer Grundbuchänderung angepingt wird
    - `aktenzeichen`: Optional[String]: Aktenzeichen, was auf Benachrichtigungen bei 
      Grundbuchänderungen an diesem Blatt später bei "Ihr Zeichen" / "Unser Zeichen" 
      auftauchen wird.

Achtung: Webhooks funktionieren aus Sicherheitsgründen nur mit HTTPS-Servern.

```
POST https://127.0.0.1/abo-neu/email/Prenzlau/Schenkenberg/289 HTTP/1.1
Authorization: Bearer S0VLU0UhIExFQ0tFUiEK
Content-Type: application/x-www-form-urlencoded
aktenzeichen=ABC%20DEF&url=meinwebhook.com:8080
```

#### Webhook neu anlegen: Ok

- `status`: String: immer `"ok"`

#### Webhook neu anlegen: Fehler

- `status`: String: immer `"error"`
- `code`: Integer: Fehlercode
    - 0: Benutzer nicht gefunden
    - 500: Interner Fehler
- `text`: String: Fehlermeldung vorformatiert als Text

#### Webhook-JSON bei Änderung

Wenn eine Grundbuchänderung am Blatt `{amtsgericht}/{grundbuchbezirk}/{blatt}` 
vorgenomment wird, wird die Änderung dem Webhook-Server gemeldet:

- `server_url`: String: URL des Servers, von dem die Benachrichtigung gesendet wurde
- `amtsgericht`: String: Amtsgericht des Grundbuchblatts, in dem die Änderung stattfand
- `grundbuchbezirk`: String: Grundbuchbezirk des Grundbuchblatts, in dem die Änderung stattfand
- `blatt`: String: Blatt-Nr., in dem die Änderung stattfand
- `webhook`: String: URL des Webhooks, der benachrichtigt wurde
- `aktenzeichen`: Optional[String]: Aktenzeichen, unter dem das Abonnement geführt wird
- `aenderungs_id`: String: Änderungs-ID der Grundbuchänderung (SHA1-Hash)

Beispiel: 

```
POST https://meinwebhook.com:8080 HTTP/1.1
Content-Type: application/json
{
    "server_url": "https://127.0.0.1",
    "amtsgericht": "Prenzlau",
    "grundbuchbezirk": "Schenkenberg",
    "blatt": "289",
    "webhook": "https://meinwebhook.com:8080",
    "aktenzeichen": "ABC DEF",
    "aenderungs_id": "c913905482d2d22befe3e0f85e93795cf8a998cc"
}
```

### Upload

Lädt eine neue Datei hoch. Hierbei muss das JSON der Änderung mit einem privaten Schlüssel
unterzeichnet werden, wobei die Signatur separat übermittelt wird (Format siehe unten).

URL: POST `/upload`

```
POST https://127.0.0.1/upload HTTP/1.1
Content-Type: application/json
Authorization: Bearer S0VLU0UhIExFQ0tFUiEK

ÄNDERUNG_JSON (siehe unten)
```

Hierbei ist `ÄNDERUNG_JSON` ein JSON-Objekt, das die Änderung beschreibt:

- `titel`: String: Beschreibung der Änderung (Überschrift)
- `beschreibung`: Array[String]: Ausführliche Beschreibung der Änderung (Zeilen)
- `fingerprint`: String: Fingerprint (Schlüssel-ID) des PGP-Schlüssels, der für die Unterschrift verwendet wurde
- `signatur`: Objekt: Signatur von `data` in JSON-Form (Format siehe Beispiel unten)
    - `hash`: String: Hashfunktion die zur Unterschrift verwendet wurde (üblicherweise "SHA512")
    - `pgp_signatur`: Array[String]: Zeilen zwischen "BEGIN PGP SIGNATURE" und "END PGP SIGNATURE"
- `data`: Objekt:
    - `neu`: Optional[Array[GbxDatei]]: Enthält Dateien, 
       die keinen alten Stand haben (z.B. neu angelegte Blätter)
    - `geaendert`: Optional[Array[Objekt]]:
        - `alt`: GbxDatei: Der alte Stand der GBX-Datei vor der Änderung
        - `neu`: GbxDatei: Der neue Stand der GBX-Datei nach der Änderung

Beispiel: In einer neu angelegten Datei wird ein neuer BV-Eintrag eingefügt.
Die leere GBX-Datei hat den Inhalt von:

```json
{
  "digitalisiert": false,
  "land": "Brandenburg",
  "inhalt": {
    "titelblatt": {
      "amtsgericht": "Prenzlau",
      "grundbuch_von": "Schenkenberg",
      "blatt": 456
    }
  }
}
```

Die geänderte Datei:

```json
{
  "digitalisiert": false,
  "land": "Brandenburg",
  "inhalt": {
    "titelblatt": {
      "amtsgericht": "Prenzlau",
      "grundbuch_von": "Schenkenberg",
      "blatt": 456
    },
    "bestandsverzeichnis": {
      "eintraege": [
        {
          "lfd_nr": 1,
          "flur": 1,
          "flurstueck": "26",
          "bezeichnung": [
            "Landwirtschaftsfläche"
          ],
          "groesse": 15035,
        }
      ]
    }
  }
}
```

Die PGP-Nachricht muss dann so aussehen (Zeilenenden = CR/LF, 
eingerückt mit 4 Leerzeichen):

```txt
-----BEGIN PGP SIGNED MESSAGE-----
Hash: SHA256
{
    "geaendert": [
        {
            "alt": {
                "digitalisiert": false,
                "land": "Brandenburg",
                "inhalt": {
                    "titelblatt": {
                        "amtsgericht": "Prenzlau",
                        "grundbuch_von": "Schenkenberg",
                        "blatt": 456
                    }
                }
            },
            "neu": {
                "digitalisiert": false,
                "land": "Brandenburg",
                "inhalt": {
                    "titelblatt": {
                        "amtsgericht": "Prenzlau",
                        "grundbuch_von": "Schenkenberg",
                        "blatt": 456
                    },
                    "bestandsverzeichnis": {
                        "eintraege": [
                            {
                                "lfd_nr": 1,
                                "flur": 1,
                                "flurstueck": "26",
                                "bezeichnung": [
                                "Landwirtschaftsfläche"
                                ],
                                "groesse": 15035,
                            }
                        ]
                    }
                }
            }
        }
    ]
}
-----BEGIN PGP SIGNATURE-----
iD8DBQFFxqRFCMEe9B/8oqERAqA2A
Tx4RziVzY4eR4Ms4MFsKAMqOoQCgg
e5AJIRuLUIUikjNWQIW63QE=J9167
=aAhry
-----END PGP SIGNATURE-----
```

Die Beschreibung (Änderungsmitteilung) ist nicht Teil der 
Nachricht selber. Das fertige JSON-Objekt sieht dann so aus:

```json
{
    "titel": "Meine Änderung 1",
    "beschreibung": [
        "Meine mehrzeilige",
        "Beschreibung der Änderung"
    ],
    "fingerprint": "F554A3687412CFFEBDEFE0A312F5F7B42F2B01E7",
    "signatur": {
        "hash": "SHA256",
        "pgp_signatur": [
            "iD8DBQFFxqRFCMEe9B/8oqERAqA2A",
            "Tx4RziVzY4eR4Ms4MFsKAMqOoQCgg",
            "e5AJIRuLUIUikjNWQIW63QE=J9167",
            "=aAhry"
        ]
    },
    "data": {
        "geaendert": [
            {
                "alt": { ... }, /* siehe oben */
                "neu": { ... }  /* siehe oben */
            }
        ]
    }
}
```

Nach dem Hochladen verifiziert der Server die Änderung gegen den öffentlichen
Schlüssel (public key) und weist Änderungen zurück, die eine falsche Unterschrift
besitzen.

#### Upload: Ok

- `status`: String: immer `"ok"`

#### Upload: Fehler

- `status`: String: immer `"error"`
- `code`: Integer: Fehlercode
    - 0: Benutzer / Passwort stimmt nicht
    - 1: Amtsgericht / Gemarkungsbezirk nicht gefunden
    - 500: Signatur stimmt nicht überein
    - 501: Interner Fehler bei Übernahme der Änderung
- `text`: String: Vorformatierter Fehlertext
