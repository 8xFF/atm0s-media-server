# Kubernetes (outdate, update needed)

You can install into kubernetes cluster by Helm chart

```bash
helm repo add 8xff https://8xff.github.io/helm
helm repo update
helm install atm0s-media-stack 8xff/atm0s-media-stack --set gateway.host={host}.{example.com} --namespace atm0s-media --create-namespace
```

#TODO need to config for both single zone or multi-zones
