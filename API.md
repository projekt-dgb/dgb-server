# dgb-server 

Server für digitale Grundbuch-Daten (.gbx)

## API-Übersicht

HTTP GET-API: `$url?email={email}&passwort={passwort}`:

```
/suche/{suchbegriff}

/download/gbx/{amtsgericht}/{grundbuch_von}/{blatt}
/download/pdf/{amtsgericht}/{grundbuch_von}/{blatt}

/abo-neu/email/{amtsgericht}/{grundbuchbezirk}/{blatt}/{aktenzeichen}
/abo-neu/webhook/{amtsgericht}/{grundbuchbezirk}/{blatt}/{aktenzeichen}

/abo-loeschen/email/{amtsgericht}/{grundbuchbezirk}/{blatt}/{aktenzeichen}
/abo-loeschen/webhook/{amtsgericht}/{grundbuchbezirk}/{blatt}/{aktenzeichen}
```

HTTP POST-API: `$url?email={email}&passwort={passwort}`:

```
/upload
```

## Beispiel

`curl https://127.0.0.1/suche/Suchbegriff?email=max@mustermann.de&passwort=geheim123`

```json
{
  "status": "ok",
  "grundbuecher": [],
  "aenderungen": []
}
```

oder: 

```json
{
  "status": "error",
  "code": 0,
  "text": "Kein Benutzer für \"max@mustermann.de\" gefunden"
}
```

## API-Dokumentation

### /suche

Durchsucht Grundbuchblätter nach einem Suchbegriff

`curl https://127.0.0.1/suche/Suchbegriff?email=max@mustermann.de&passwort=geheim123`

OK:

- `status`: String: immer `"ok"`
- `grundbuecher`: Array[Objekt]: Grundbücher, die den Suchbegriff enthalten (max. 50 Ergebnisse)
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
    - `abos`: Array[Objekt]: Abonnements für dieses Grundbuchblatt (ein Blatt kann unter mehreren AZ abonniert sein)
        - `amtsgericht`: String: Amtsgericht des Abonnements
        - `grundbuchbezirk`: String: Grundbuchbezirk des Abonnements
        - `blatt`: String: Blatt-Nr. des Abonnements
        - `text`: String: E-Mail des Abonnenten (`= "max@mustermann.de"`)
        - `aktenzeichen`: String: Aktenzeichen des Abonnements

FEHLER:

- `status`: String: immer `"error"`
- `code`: Integer: Fehlercode
    - 0: Benutzer nicht gefunden
    - 500: Interner Fehler
- `text`: String: Fehlermeldung vorformatiert als Text

### /download/pdf/{amtsgericht}/{grundbuch_von}/{blatt}

Rendert den momentanen Stand des Grundbuchs in eine PDF-Datei

`curl https://127.0.0.1/download/pdf/Prenzlau/Schenkenberg/289?email=max@mustermann.de&passwort=geheim123`

OK: PDF-Datei im Format `application/pdf`

FEHLER: 

- `status`: String: immer `"error"`
- `code`: Integer: Fehlercode
    - 0: Benutzer nicht gefunden
    - 1: Ungültiges Amtsgericht / ungültiger Gemarkungsbezirk
    - 404: Grundbuchblatt existiert (noch) nicht
    - 500: Interner Fehler
- `text`: String: Fehlermeldung vorformatiert als Text

### /download/gbx/{amtsgericht}/{grundbuch_von}/{blatt}

Gibt den momentanen Stand des Grundbuchs im JSON-Format (.gbx) zurück

`curl https://127.0.0.1/download/pdf/Prenzlau/Schenkenberg/289?email=max@mustermann.de&passwort=geheim123`

OK: 

- `status`: String: immer `"ok"`
- `datei`, `gbx_datei_pfad`, `titelblatt`, ...: GBX-Datei, Format siehe unten

FEHLER: 

- `status`: String: immer `"error"`
- `code`: Integer: Fehlercode
    - 0: Benutzer nicht gefunden
    - 1: Ungültiges Amtsgericht / ungültiger Gemarkungsbezirk
    - 404: Grundbuchblatt existiert (noch) nicht
    - 500: Interner Fehler
- `text`: String: Fehlermeldung vorformatiert als Text

### /abo-neu/email/{amtsgericht}/{grundbuchbezirk}/{blatt}/{aktenzeichen}/{email}

Legt ein neues E-Mail-Abonnement für den Benutzer an

`curl https://127.0.0.1/abo-neu/email/Prenzlau/Schenkenberg/289/max@mustermann.de?email=max@mustermann.de&passwort=geheim123`

OK: 

- `status`: String: immer `"ok"`

FEHLER: 

- `status`: String: immer `"error"`
- `code`: Integer: Fehlercode
    - 0: Benutzer nicht gefunden
    - 500: Interner Fehler
- `text`: String: Fehlermeldung vorformatiert als Text


### /abo-neu/webhook/{amtsgericht}/{grundbuchbezirk}/{blatt}/{aktenzeichen}

Legt ein neues Webhook-Abonnement für den Benutzer an: In diesem Fall wird bei
einer Änderung "https://meinwebhook.com:8080" benachrichtigt.

`curl https://127.0.0.1/abo-neu/email/Prenzlau/Schenkenberg/289/meinwebhook.com:8080?email=max@mustermann.de&passwort=geheim123`

OK: 

- `status`: String: immer `"ok"`

FEHLER: 

- `status`: String: immer `"error"`
- `code`: Integer: Fehlercode
    - 0: Benutzer nicht gefunden
    - 500: Interner Fehler
- `text`: String: Fehlermeldung vorformatiert als Text

Webhook-JSON bei Änderung:

AN: HTTP POST https://meinwebhook.com:8080

INHALT: 

- `server_url`: String: URL des Servers, von dem die Benachrichtigung gesendet wurde
- `amtsgericht`: String: Amtsgericht des Grundbuchblatts, in dem die Änderung stattfand
- `grundbuchbezirk`: String: Grundbuchbezirk des Grundbuchblatts, in dem die Änderung stattfand
- `blatt`: String: Blatt-Nr., in dem die Änderung stattfand
- `webhook`: String: URL des Webhooks, der benachrichtigt wurde
- `aktenzeichen`: String: Aktenzeichen, unter dem das Abonnement geführt wird
- `aenderungs_id`: String: Änderungs-ID der Grundbuchänderung (SHA1-Hash)

### /upload

Lädt eine neue Datei hoch. Hierbei muss das JSON der Änderung mit einem privaten Schlüssel
unterzeichnet werden, wobei die Signatur separat übermittelt wird (Format siehe unten).

```
curl -X POST https://127.0.0.1/upload?email=max@mustermann.de&passwort=geheim123
   -H 'Content-Type: application/json'
   -d 'ÄNDERUNG_JSON (siehe unten)'
```

Hierbei ist `ÄNDERUNG_JSON` ein JSON-Objekt, das die Änderung beschreibt:

- `titel`: String: Beschreibung der Änderung (Überschrift)
- `beschreibung`: Array[String]: Ausführliche Beschreibung der Änderung (Zeilen)
- `fingerprint`: String: Fingerprint (Schlüssel-ID) des PGP-Schlüssels, der für die Unterschrift verwendet wurde
- `signatur`: Objekt: Signatur von `data` in JSON-Form (Format siehe Beispiel unten)
    - `hash`: String: Hashfunktion die zur Unterschrift verwendet wurde (üblicherweise "SHA512")
    - `pgp_signatur`: Array[String]: Zeilen zwischen "BEGIN PGP SIGNATURE" und "END PGP SIGNATURE"
- `data`: Objekt:
    - `neu`: Array[GbxDatei]: Enthält Dateien, die keinen alten Stand haben (z.B. neu angelegte Blätter)
    - `geaendert`: Array[Objekt]:
        - `alt`: GbxDatei: Der alte Stand der GBX-Datei vor der Änderung
        - `neu`: GbxDatei: Der neue Stand der GBX-Datei nach der Änderung

Beispiel: In einer neu angelegten Datei wird ein neuer BV-Eintrag eingefügt.
Die leere GBX-Datei hat den Inhalt von:

```json
{
  "gbx_datei_pfad": "",
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
  "gbx_datei_pfad": "",
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
          "groesse": {
            "typ": "m",
            "wert": {
              "m2": 15035
            }
          }
        }
      ]
    }
  }
}
```

Die PGP-Nachricht muss dann so aussehen (Zeilenenden = CR/LF):

```txt
-----BEGIN PGP SIGNED MESSAGE-----
Hash: SHA256
{
    "neu": [],
    "geaendert": [
        {
            "alt": {
                "gbx_datei_pfad": "",
                "land": "Brandenburg",
                "titelblatt": {
                    "amtsgericht": "Prenzlau",
                    "grundbuch_von": "Schenkenberg",
                    "blatt": 456
                },
                "analysiert": {
                  "titelblatt": {
                      "amtsgericht": "Prenzlau",
                      "grundbuch_von": "Schenkenberg",
                      "blatt": 456
                  }
                }
            },
            "neu": {
                "gbx_datei_pfad": "",
                "land": "Brandenburg",
                "titelblatt": {
                    "amtsgericht": "Prenzlau",
                    "grundbuch_von": "Schenkenberg",
                    "blatt": 456
                },
                "analysiert": {
                    "titelblatt": {
                        "amtsgericht": "Prenzlau",
                        "grundbuch_von": "Schenkenberg",
                        "blatt": 456
                    },
                    "bestandsverzeichnis": {
                        "eintraege": [
                            {
                                "lfd_nr": 1,
                                "bisherige_lfd_nr": null,
                                "flur": 1,
                                "flurstueck": "26",
                                "gemarkung": null,
                                "bezeichnung": [
                                    "Landwirtschaftsfläche"
                                ],
                                "groesse": {
                                    "typ": "m",
                                    "wert": {
                                        "m2": 15035
                              	    }
                                },
                                "automatisch_geroetet": null,
                                "manuell_geroetet": null,
                                "position_in_pdf": null
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

Die Beschreibung (Änderungsmitteilung) ist nicht Teil der Nachricht selber.

Das fertige JSON-Objekt sieht dann so aus:

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
        "neu": [],
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

OK:

Bei Übernahme der Änderung schickt der Sever als Bestätigung das `data`-Objekt
nochmal zurück zur Überprüfung.

- `status`: String: immer `"ok"`
- `neu`: Array[GbxDatei]: siehe oben
- `geaendert`: Array[GbxDatei]: siehe oben

FEHLER:

- `status`: String: immer `"error"`
- `code`: Integer: Fehlercode
    - 0: Benutzer / Passwort stimmt nicht
    - 1: Amtsgericht / Gemarkungsbezirk nicht gefunden
    - 500: Signatur stimmt nicht überein
    - 501: Interner Fehler bei Übernahme der Änderung
- `text`: String: Vorformatierter Fehlertext
