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

# cert-manager installieren
kubectl apply -f ./k8s/1.yaml
kubectl apply -f ./k8s/2.yaml
kubectl apply -f ./k8s/3.yaml
kubectl apply -f ./k8s/4.yaml

# Wichtigt: jetzt die IP-Addresse des Ingress in DNS-Einstellungen einstellen, bevor ACME-Challenge läuft
# "grundbuch-test.eu" -> "meine.website"
kubectl apply -f ./k8s/5.yml
```

Jetzt sollte der Server über "https://meine.website" erreichbar 
sein.
