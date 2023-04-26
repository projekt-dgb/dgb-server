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
export KUBECONFIG=./k8s/kubeconfig.yaml

# Startet den Server auf :80 und :443 (SSL_PROTOCOL_ERROR)
# "grundbuch-test.eu" -> "meine.website"
kubectl apply -f ./k8s/deploy.yaml

# Jetzt die IP-Addresse des LoadBalancers in DNS-Einstellungen einstellen
curl -i https://meine.website
```

Jetzt sollte der Server über "https://meine.website" erreichbar 
sein.
