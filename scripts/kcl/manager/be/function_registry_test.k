import .function_registry as fr
import k8s.api.core.v1 as corev1
import external_secrets.v1 as es

test_create_namespace = lambda {
    _ns = fr.create_namespace({name: "test-ns", labels: {"env": "test"}})

    assert _ns.metadata.name == "test-ns"
    assert _ns.metadata.labels.env == "test"
}

test_create_secret = lambda {
    _secret = fr.create_secret({
        name: "test-secret"
        namespace: "default"
        data: {"key": "value"}
        labels: {"app": "test"}
    })

    assert _secret.metadata.name == "test-secret"
    assert _secret.metadata.namespace == "default"
    assert _secret.data.key == "value"
    assert _secret.metadata.labels.app == "test"
}

test_create_cluster_secret_store = lambda {
    _store = fr.create_cluster_secret_store({
        name: "test-store"
        projectID: "test-project"
        secretName: "test-creds"
        secretKey: "creds"
        namespaces: ["default", "apps"]
    })

    assert _store.metadata.name == "test-store"
    assert _store.spec.provider.gcpsm.projectID == "test-project"
    assert _store.spec.provider.gcpsm.auth.secretRef.secretAccessKeySecretRef.name == "test-creds"
    assert _store.spec.conditions[0].namespaces == ["default", "apps"]
}

test_create_cluster_external_secret = lambda {
    _es = fr.create_cluster_external_secret({
        name: "test-external-secret"
        secretStoreName: "test-store"
        externalSecretName: "test-secret"
        refreshTime: "10m"
        secrets: ["secret1", "secret2"]
    })

    assert _es.metadata.name == "test-external-secret"
    assert _es.spec.externalSecretName == "test-secret"
    assert _es.spec.refreshTime == "10m"
    assert _es.spec.externalSecretSpec.secretStoreRef.name == "test-store"
    assert len(_es.spec.externalSecretSpec.dataFrom) == 2
}

test_create_xrd = lambda {
    _xrd = fr.create_xrd({
        kind: "Database"
        plural: "databases"
        group: "example.com"
        claimKind: "DatabaseClaim"
        claimPlural: "databaseclaims"
        owner: "test@example.com"
    })

    assert _xrd.metadata.name == "databases.example.com"
    assert _xrd.spec.group == "example.com"
    assert _xrd.spec.names.kind == "Database"
    assert _xrd.spec.claimNames.kind == "DatabaseClaim"
}

test_registry_register = lambda {
    _custom_fn = lambda input: any -> corev1.ConfigMap {
        corev1.ConfigMap {
            metadata.name: input.name
        }
    }

    _new_registry = fr.register_function(fr.registry, "create_configmap", _custom_fn)

    assert "create_configmap" in fr.list_functions(_new_registry)
}

test_registry_execute = lambda {
    _result = fr.execute_function(fr.registry, "create_namespace", {name: "executed-ns"})

    assert _result.metadata.name == "executed-ns"
}

test_registry_list = lambda {
    _functions = fr.list_functions(fr.registry)

    assert "create_namespace" in _functions
    assert "create_secret" in _functions
    assert "create_cluster_secret_store" in _functions
    assert "create_cluster_external_secret" in _functions
    assert "create_xrd" in _functions
}

test_registry_execute_nonexistent = lambda {
    _failed = False
    _result = None
    if "nonexistent" not in fr.list_functions(fr.registry):
        _failed = True

    assert _failed == True
}

test_create_namespace()
test_create_secret()
test_create_cluster_secret_store()
test_create_cluster_external_secret()
test_create_xrd()
test_registry_register()
test_registry_execute()
test_registry_list()
test_registry_execute_nonexistent()
