[package]
name = "manager"
edition = "v0.11.2"
version = "0.0.1"

[dependencies]
external-secrets = "0.18.2"
k8s = "1.32.4"
stam = { oci = "oci://docker.io/yurikrupnik/kcl-stam", tag = "0.0.7", version = "0.0.7" }
crossplane = "v2.0.2"
