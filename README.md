# dgb-server

## Installation

### Entwicklung 

```sh
cargo run --release -- start && open 127.0.0.1:8080
```

### Deployment

```sh
docker build -t grundbuch/dgb-server:latest .
docker run -p 127.0.0.1:80:8080 grundbuch/dgb-server:latest
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
Cloudflare-Konto ein API-Schlüssel aktiviert sein.

Hinweis: Für die Produktion ist Cloudflare nicht zu empfehlen, aber für das Testen
reicht es völlig aus.

```
# CLOUDFLARE_API_TOKEN = token
# DOCKER_CONFIG_JSON = base64(.dockerconfigjson)
cp ./k8s/secret.template.yml ./k8s/secret.yml
kubectl apply -f ./k8s/secrets.yml
# "test-grundbuch.eu" -> "meine.website"
kubectl apply -f ./k8s/deploy-addssl.yml
```

Jetzt sollte der Server über "https://meine.website" erreichbar 
sein.
