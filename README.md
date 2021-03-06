# dgb-server

## Installation

### Entwicklung 

```sh
cargo run --release -- start && open 127.0.0.1:8080
```

### Deployment

```sh
sudo apt install musl-tools lld
rustup override set 1.60.0
rustup target add x86_64-unknown-linux-musl

cargo build --release --target x86_64-unknown-linux-musl
docker build -t grundbuch/dgb-server:latest .
docker login
docker push grundbuch/dgb-server:latest
```

### SSL

Für das Deployment einer SSL-gesicherten Verbindung benötigt 
man einen Domainnamen (in deploy.yml = "test-grundbuch.eu")
sowie eine `kubeconfig.yaml` Datei zum Anmelden im Kubernetes-Cluster.

```
# K8s cluster ohne SSL erstellen
export KUBECONFIG=./k8s/kubeconfig.yaml
kubectl create -f ./k8s/deploy-nossl.yml

# cert-manager installieren
kubectl create ns cert-manager
kubectl apply -f ./k8s/cert-manager.crds.yaml
kubectl apply -f ./k8s/cert-manager.yaml

# Neues Deployment
kubectl rollout restart deployment dgb-server
```

Zum Installieren der LetsEncrypt-SSL-Zertifikate benötigt `cert-manager`
Zugriff auf die Cloudflare-API zum Erstellen der `__acme-challenge` DNS-Einträge, 
siehe https://link.medium.com/aQ4vqJ5Fjrb. Hierbei muss im 
Cloudflare-Konto ein API-Schlüssel.

Wenn man sich mit diesem Einmal-Passwort anmeldet, können Benutzerkonten 
verwaltet / angelegt / archiviert werden, ansonsten nicht. Konten werden
nie gelöscht, da ansonsten der PublicKey verloren geht, welcher zum 
Verifizieren der Grundbucheinträge benötigt wird.

```
# CLOUDFLARE_API_TOKEN = token
# DOCKER_CONFIG_JSON = base64(.dockerconfigjson)
# root-access.* = ausfüllen
cp ./k8s/secret.template.yml ./k8s/secret.yml
kubectl apply -f ./k8s/secrets.yml
# "test-grundbuch.eu" -> "meine.website"
kubectl apply -f ./k8s/deploy-addssl.yml
```

Jetzt sollte der Server über "https://meine.website" erreichbar 
sein.