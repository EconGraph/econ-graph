"""Offline guard/lifecycle tests. No real host or existing cluster is contacted."""

import argparse
from contextlib import contextmanager, redirect_stdout
import copy
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import cluster
import common
import host


EXAMPLE = common.ROOT / "infra/talos/example.cluster.json"


class TalosTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="econgraph-talos-tests-")
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        self.config = common.load_config(EXAMPLE)
        self.state = self.directory / "state"

    def args(self, action, **values):
        defaults = dict(action=action, execute=False, confirm_owner=None, age=[], host_state_dir=None, destroy_disposable=None, node=None, namespace=None, deployment=None, image=None, kubernetes_version=None, debug_image=None, evidence=None)
        defaults.update(values)
        return argparse.Namespace(**defaults)

    def altered_config(self, change):
        data = json.loads(EXAMPLE.read_text())
        change(data)
        source = self.directory / "config.json"
        source.write_text(json.dumps(data))
        return source

    def test_all_lifecycle_commands_are_dry_run_without_tools_or_files(self):
        for action in ("secrets", "build", "rebuild", "credentials", "status", "upgrade-talos", "upgrade-kubernetes", "kill-worker", "restart-worker", "debug"):
            with self.subTest(action=action), patch.object(cluster, "call", side_effect=AssertionError("dry run executed a command")), redirect_stdout(io.StringIO()) as output:
                cluster.operate(self.args(action), self.config, self.state)
                self.assertIn("DRY RUN", output.getvalue())
                self.assertFalse(self.state.exists())

    def test_host_dry_run_never_executes_or_writes(self):
        with patch.object(host, "call", side_effect=AssertionError("host command executed")), redirect_stdout(io.StringIO()) as output:
            host.prepare(self.config, self.state)
        self.assertIn("chattr +C", output.getvalue())
        self.assertIn("unused-LAN-uplink", output.getvalue())
        self.assertFalse(self.state.exists())

    def test_execute_requires_matching_owner(self):
        with self.assertRaisesRegex(common.Refused, "confirm-owner"):
            common.execute_guard(self.args("build", execute=True, confirm_owner="different"), self.config)

    def test_example_addresses_cannot_be_executed(self):
        with self.assertRaisesRegex(common.Refused, "documentation-only"):
            common.execute_guard(self.args("build", execute=True, confirm_owner=self.config["owner_id"]), self.config)

    def test_duplicate_ip_is_refused(self):
        source = self.altered_config(lambda data: data["nodes"]["cp-2"].update(ip=data["nodes"]["cp-1"]["ip"]))
        with self.assertRaisesRegex(common.Refused, "unique"):
            common.load_config(source)

    def test_network_overlap_is_refused(self):
        source = self.altered_config(lambda data: data.update(pod_cidr="192.0.2.0/24"))
        with self.assertRaisesRegex(common.Refused, "overlap"):
            common.load_config(source)

    def test_missing_control_plane_is_refused(self):
        source = self.altered_config(lambda data: data["nodes"].pop("cp-3"))
        with self.assertRaisesRegex(common.Refused, "control planes"):
            common.load_config(source)

    def test_scratch_owner_cannot_authorize_production_inventory(self):
        source = self.altered_config(lambda data: data.update(purpose="production"))
        with self.assertRaisesRegex(common.Refused, "scratch_owner"):
            common.load_config(source)

    def test_scratch_owner_must_match_explicit_operational_owner(self):
        source = self.altered_config(lambda data: data.update(scratch_owner="different"))
        with self.assertRaisesRegex(common.Refused, "must match owner_id"):
            common.load_config(source)

    def test_ordinary_production_inventory_gets_no_implicit_scratch_authorization(self):
        def production(data):
            data["purpose"] = "production"
            data.pop("scratch_owner")
        config = common.load_config(self.altered_config(production))
        self.assertNotIn("scratch_owner", config)
        module = cluster.stage_module(self.state, config)
        value = json.loads((module / "terraform.tfvars.json").read_text())["cluster"]
        self.assertEqual(value["purpose"], "production")
        self.assertIsNone(value["scratch_owner"])

    def test_runtime_under_repository_is_refused(self):
        with self.assertRaisesRegex(common.Refused, "outside the checkout"):
            common.external_path(str(common.ROOT / "private-state"))

    def test_symlink_runtime_is_refused(self):
        path = self.directory / "alias"
        path.symlink_to(self.directory, target_is_directory=True)
        with self.assertRaisesRegex(common.Refused, "symlinks"):
            common.external_path(str(path))

    def test_runtime_claim_is_idempotent_and_refuses_foreign_owner(self):
        common.claim(self.state, self.config)
        common.claim(self.state, self.config)
        other = dict(self.config, owner_id="2cf95ff1-e1f9-41ec-a18a-a75d65791561")
        with self.assertRaisesRegex(common.Refused, "ownership"):
            common.claim(self.state, other)

    def test_existing_unowned_directory_is_not_adopted(self):
        self.state.mkdir()
        with self.assertRaisesRegex(common.Refused, "refusing adoption"):
            common.claim(self.state, self.config)

    def test_active_management_uplink_is_refused(self):
        for addresses, routes in (([{"addr_info": [{"scope": "global"}]}], []), ([{"addr_info": []}], [{"gateway": "192.168.1.1"}])):
            with self.subTest(addresses=addresses, routes=routes), self.assertRaisesRegex(common.Refused, "management-network"):
                host.unused_uplink([{}], addresses, routes, "br-eg-fit")

    def test_foreign_bridge_uplink_is_refused(self):
        with self.assertRaisesRegex(common.Refused, "another bridge"):
            host.unused_uplink([{"master": "production-bridge"}], [{"addr_info": []}], [], "br-eg-fit")

    def test_owned_unused_uplink_can_be_rechecked(self):
        self.assertEqual(host.unused_uplink([{"master": "br-eg-fit"}], [{"addr_info": []}], [], "br-eg-fit"), "br-eg-fit")

    def test_existing_management_lan_route_is_refused(self):
        with self.assertRaisesRegex(common.Refused, "another host interface"):
            host.lan_routes([{"dev": "management", "dst": "192.0.2.0/24"}], self.config)
        host.lan_routes([{"dev": "br-eg-fit", "dst": "192.0.2.0/24"}], self.config)

    def test_new_image_parents_are_traversable_without_changing_existing_parents(self):
        existing = self.directory / "existing"
        existing.mkdir(mode=0o700)
        path = existing / "new" / "images"
        host.new_parent_directories(path)
        self.assertEqual(path.stat().st_mode & 0o777, 0o755)
        self.assertEqual(path.parent.stat().st_mode & 0o777, 0o755)
        self.assertEqual(existing.stat().st_mode & 0o777, 0o700)

    def test_anonymous_memory_file_closes_without_plaintext_disk_artifact(self):
        content = b"fake-secret-for-memory-file-test"
        with cluster.memory_file(content) as path:
            self.assertEqual(Path(path).read_bytes(), content)
        self.assertFalse(Path(path).exists())
        self.assertEqual(list(self.directory.iterdir()), [])

    def test_failed_command_does_not_expose_captured_credentials(self):
        result = subprocess.CompletedProcess(["sops"], 1, b"secret-output", b"secret-error")
        with patch.object(common.subprocess, "run", return_value=result):
            with self.assertRaisesRegex(common.Refused, "output withheld") as error:
                common.call(["sops", "--decrypt", "ciphertext"])
        self.assertNotIn("secret-output", str(error.exception))
        self.assertNotIn("secret-error", str(error.exception))

    def test_command_timeout_is_bounded_and_does_not_expose_output(self):
        with patch.object(common.subprocess, "run", side_effect=subprocess.TimeoutExpired(["sops"], 300, output=b"secret")) as process:
            with self.assertRaisesRegex(common.Refused, "did not complete"):
                common.call(["sops", "--decrypt", "ciphertext"])
        self.assertEqual(process.call_args.kwargs["timeout"], 300)

    def test_secrets_persist_only_sealed_content(self):
        plaintext = b"fake-talos-private-key"
        with patch.object(cluster, "call", return_value=plaintext), patch.object(cluster, "seal", return_value=b"SOPS-CIPHERTEXT"), redirect_stdout(io.StringIO()):
            cluster.make_secrets(self.state, self.config, ["age1testrecipient"])
        for path in self.state.rglob("*"):
            if path.is_file():
                self.assertNotIn(plaintext, path.read_bytes())
        self.assertEqual((self.state / "secrets.sops.yaml").read_bytes(), b"SOPS-CIPHERTEXT")
        self.assertEqual((self.state / "state-key.sops.json").stat().st_mode & 0o777, 0o600)

    def test_initial_secret_import_does_not_require_existing_state_file(self):
        module = self.directory / "module"
        module.mkdir()
        calls = []
        def tofu(*args, **kwargs):
            calls.append(args)
            self.assertEqual(args[1], "import")
            self.assertEqual(Path(args[-1]).read_bytes(), b"fake-secrets")
            return b""
        with patch.object(cluster, "decrypt", return_value=b"fake-secrets"), patch.object(cluster, "tofu", side_effect=tofu):
            cluster.import_secrets(module, self.state, {})
        self.assertEqual(len(calls), 1)

    def test_domain_ownership_is_checked_before_virsh_operation(self):
        name = self.config["name"] + "-worker-1"
        xml = f"<domain><name>{name}</name><description>production VM</description></domain>".encode()
        with patch.object(cluster, "call", return_value=xml):
            with self.assertRaisesRegex(common.Refused, "domain ownership"):
                cluster.verify_domain(self.config, "worker-1")

    def test_owned_vm_with_foreign_attached_disk_is_refused(self):
        name = self.config["name"] + "-worker-1"
        owner = f"econgraph-talos owner={self.config['owner_id']} cluster={self.config['name']} role=worker"
        xml = f'<domain><name>{name}</name><description>{owner}</description><devices><disk><source file="/srv/primary/postgres.qcow2"/></disk></devices></domain>'.encode()
        with patch.object(cluster, "call", return_value=xml):
            with self.assertRaisesRegex(common.Refused, "outside the owned"):
                cluster.verify_domain(self.config, "worker-1")

    def test_state_uuid_cannot_redirect_destroy_to_another_vm(self):
        name = self.config["name"] + "-worker-1"
        owner = f"econgraph-talos owner={self.config['owner_id']} cluster={self.config['name']} role=worker"
        xml = f'<domain><name>{name}</name><uuid>live-owned-uuid</uuid><description>{owner}</description></domain>'.encode()
        with patch.object(cluster, "call", return_value=xml):
            with self.assertRaisesRegex(common.Refused, "UUID does not match"):
                cluster.verify_domain(self.config, "worker-1", "different-vm-uuid")

    def test_state_volume_key_cannot_redirect_deletion_outside_owned_path(self):
        owned_path = self.config["disk_path"] + "/owned.qcow2"
        resource = {"mode": "managed", "address": 'libvirt_volume.data["worker-1"]', "name": "data", "index": "worker-1", "type": "libvirt_volume", "values": {"pool": self.config["name"], "path": owned_path, "id": "/srv/primary/foreign.qcow2", "key": "/srv/primary/foreign.qcow2"}}
        state = {"values": {"root_module": {"resources": [resource]}}}
        with patch.object(cluster, "tofu", return_value=json.dumps(state).encode()):
            with self.assertRaisesRegex(common.Refused, "volume key differs"):
                cluster.inspect_state(self.directory, self.config, {})

    def test_live_pool_drift_blocks_rebuild(self):
        state = {"values": {"root_module": {"resources": [{"mode": "managed", "address": "libvirt_pool.cluster", "name": "cluster", "type": "libvirt_pool", "values": {"name": self.config["name"], "target": {"path": self.config["disk_path"]}}}]}}}
        xml = f'<pool><name>{self.config["name"]}</name><target><path>/srv/primary</path></target></pool>'.encode()
        with patch.object(cluster, "tofu", return_value=json.dumps(state).encode()), patch.object(cluster, "call", return_value=xml):
            with self.assertRaisesRegex(common.Refused, "live storage pool"):
                cluster.inspect_state(self.directory, self.config, {})

    def test_foreign_volume_in_state_blocks_rebuild(self):
        state = {"values": {"root_module": {"resources": [{"mode": "managed", "address": 'libvirt_volume.data["worker-1"]', "name": "data", "index": "worker-1", "type": "libvirt_volume", "values": {"pool": "primary", "path": "/srv/primary/data.qcow2"}}]}}}
        with patch.object(cluster, "tofu", return_value=json.dumps(state).encode()):
            with self.assertRaisesRegex(common.Refused, "outside the owned"):
                cluster.inspect_state(self.directory, self.config, {})

    def test_extra_runtime_module_cannot_expand_apply_scope(self):
        common.claim(self.state, self.config)
        module = self.state / "tofu"
        module.mkdir()
        (module / "foreign.tf").write_text('resource "foreign" "dangerous" {}')
        with self.assertRaisesRegex(common.Refused, "extra infrastructure"):
            cluster.stage_module(self.state, self.config)

    def test_native_tofu_overrides_cannot_shadow_pinned_module(self):
        common.claim(self.state, self.config)
        module = self.state / "tofu"
        module.mkdir()
        for filename in ("main.tofu", "foreign.tofu.json", "versions.tofu", "secrets.auto.tfvars", "tofu.tfvars"):
            with self.subTest(filename=filename):
                path = module / filename
                path.write_text("foreign override")
                with self.assertRaisesRegex(common.Refused, "extra infrastructure"):
                    cluster.stage_module(self.state, self.config)
                path.unlink()

    def test_foreign_talos_resource_address_blocks_rebuild(self):
        resource = {"mode": "managed", "type": "talos_machine_configuration_apply", "address": "talos_machine_configuration_apply.foreign", "name": "foreign", "values": {"node": "198.51.100.99", "on_destroy": {"reset": True}}}
        state = {"values": {"root_module": {"resources": [resource]}}}
        with patch.object(cluster, "tofu", return_value=json.dumps(state).encode()):
            with self.assertRaisesRegex(common.Refused, "unexpected resource address"):
                cluster.inspect_state(self.directory, self.config, {})

    def test_foreign_talos_node_and_reset_policy_block_rebuild(self):
        for overrides in ({"node": "198.51.100.99"}, {"endpoint": "198.51.100.99"}, {"on_destroy": {"reset": True}}):
            with self.subTest(overrides=overrides):
                values = {"node": self.config["nodes"]["worker-1"]["ip"], **overrides}
                resource = {"mode": "managed", "type": "talos_machine_configuration_apply", "address": 'talos_machine_configuration_apply.node["worker-1"]', "name": "node", "index": "worker-1", "values": values}
                state = {"values": {"root_module": {"resources": [resource]}}}
                with patch.object(cluster, "tofu", return_value=json.dumps(state).encode()):
                    with self.assertRaisesRegex(common.Refused, "outside the explicit|on_destroy reset"):
                        cluster.inspect_state(self.directory, self.config, {})

    def test_nested_module_directory_symlink_does_not_modify_its_target(self):
        common.claim(self.state, self.config)
        target = self.directory / "foreign"
        target.mkdir()
        (self.state / "tofu").symlink_to(target, target_is_directory=True)
        with self.assertRaisesRegex(common.Refused, "symlinks"):
            cluster.stage_module(self.state, self.config)
        self.assertEqual(list(target.iterdir()), [])

    def test_module_file_symlink_does_not_overwrite_its_target(self):
        common.claim(self.state, self.config)
        module = self.state / "tofu"
        module.mkdir()
        target = self.directory / "foreign.tf"
        target.write_text("do not overwrite")
        (module / "main.tf").symlink_to(target)
        with self.assertRaisesRegex(common.Refused, "symlinks"):
            cluster.stage_module(self.state, self.config)
        self.assertEqual(target.read_text(), "do not overwrite")

    def test_environment_cannot_override_encrypted_state_or_apply_scope(self):
        for key, value in (("TF_ENCRYPTION", "unencrypted"), ("TF_CLI_ARGS_apply", "-var-file=other.json"), ("TF_WORKSPACE", "other"), ("TF_LOG_PATH", "/tmp/plaintext.log")):
            with self.subTest(key=key), patch.dict(cluster.os.environ, {key: value}), self.assertRaisesRegex(common.Refused, "overrides"):
                cluster.tofu_environment(self.state)

    def test_wrong_kubernetes_endpoint_is_refused(self):
        value = {"clusters": [{"cluster": {"server": "https://primary.invalid:6443"}}]}
        with patch.object(cluster, "call", return_value=json.dumps(value).encode()):
            with self.assertRaisesRegex(common.Refused, "different API"):
                cluster.check_kube_target(["kubectl"], self.config)

    def test_worker_loss_never_targets_production(self):
        config = dict(self.config, purpose="production")
        with patch.object(cluster, "call") as command:
            with self.assertRaisesRegex(common.Refused, "fit cluster"):
                cluster.kill_worker(self.args("kill-worker", node="worker-1"), config, [], ["kubectl"])
        command.assert_not_called()

    def test_worker_loss_rejects_control_plane(self):
        with self.assertRaisesRegex(common.Refused, "explicit worker"):
            cluster.kill_worker(self.args("kill-worker", node="cp-1"), self.config, [], ["kubectl"])

    def test_worker_loss_requires_target_workload_on_victim(self):
        deployment = {"spec": {"replicas": 2, "selector": {"matchLabels": {"app": "probe"}}}, "status": {"readyReplicas": 2}}
        responses = [json.dumps(deployment).encode(), b'{"items": []}']
        with patch.object(cluster, "verify_domain", return_value=self.config["name"] + "-worker-1"), patch.object(cluster, "call", side_effect=responses) as command:
            with self.assertRaisesRegex(common.Refused, "no pod on the selected worker"):
                cluster.kill_worker(self.args("kill-worker", node="worker-1", namespace="fit", deployment="probe"), self.config, [], ["kubectl"])
        self.assertFalse(any("destroy" in item.args[0] for item in command.call_args_list))

    def test_worker_loss_observes_replacement_without_deleting_disks(self):
        name = self.config["name"] + "-worker-1"
        deployment = {"spec": {"replicas": 2, "selector": {"matchLabels": {"app": "probe"}}}, "status": {"readyReplicas": 2}}
        before = {"items": [{"spec": {"nodeName": name}}]}
        pod = {"metadata": {}, "spec": {"nodeName": self.config["name"] + "-worker-2"}, "status": {"conditions": [{"type": "Ready", "status": "True"}]}}
        responses = [json.dumps(deployment).encode(), json.dumps(before).encode(), b"", json.dumps({"items": [pod, copy.deepcopy(pod)]}).encode()]
        with patch.object(cluster, "verify_domain", return_value=name), patch.object(cluster, "call", side_effect=responses) as command, redirect_stdout(io.StringIO()):
            cluster.kill_worker(self.args("kill-worker", node="worker-1", namespace="fit", deployment="probe"), self.config, [], ["kubectl"])
        self.assertEqual(command.call_args_list[2].args[0], ["virsh", "--connect", "qemu:///system", "destroy", name])
        self.assertFalse(any("vol-delete" in item.args[0] or "undefine" in item.args[0] for item in command.call_args_list))

    def test_readiness_checks_every_inventory_node_explicitly(self):
        with patch.object(cluster, "call", return_value=b"") as command:
            cluster.wait_ready(["kubectl"], self.config)
        self.assertEqual(command.call_count, 5)
        names = [item.args[0][2] for item in command.call_args_list]
        self.assertEqual(set(names), {"node/" + self.config["name"] + "-" + name for name in self.config["nodes"]})

    def fit_evidence(self, hours=0):
        checks = {name: {"result": "passed", "evidence": "retained-artifact", "completed_at": "2026-10-30T20:00:00Z"} for name in ("build", "rebuild", "talos_upgrade", "kubernetes_upgrade", "worker_loss", "pod_debug", "release_e2e_during_drain", "fresh_cluster_restore")}
        value = {"cluster": common.identity(self.config), "talos_debugging_hours": hours, "checks": checks}
        path = self.directory / "fit.json"
        path.write_text(json.dumps(value))
        return path

    def test_fit_gate_refuses_unobserved_acceptance(self):
        path = self.fit_evidence()
        value = json.loads(path.read_text())
        value["checks"]["fresh_cluster_restore"]["result"] = "pending"
        path.write_text(json.dumps(value))
        with self.assertRaisesRegex(common.Refused, "fresh_cluster_restore"):
            cluster.fit_gate(path, self.config)

    def test_fit_gate_requires_rke2_decision_after_two_engineering_weeks(self):
        with self.assertRaisesRegex(common.Refused, "RKE2/Ubuntu"):
            cluster.fit_gate(self.fit_evidence(80), self.config)

    def test_fit_gate_rejects_nonfinite_effort(self):
        with self.assertRaisesRegex(common.Refused, "nonnegative"):
            cluster.fit_gate(self.fit_evidence(float("nan")), self.config)

    def test_fit_gate_complete_record_does_not_deploy(self):
        with patch.object(cluster, "call", side_effect=AssertionError("fit gate deployed")), redirect_stdout(io.StringIO()):
            cluster.fit_gate(self.fit_evidence(), self.config)


if __name__ == "__main__":
    unittest.main()
