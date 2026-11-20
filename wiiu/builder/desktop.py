"""MCHorizonWiiU Windows desktop builder."""
import argparse
from pathlib import Path
import queue
import subprocess
import threading
import tkinter as tk
from tkinter import messagebox, ttk

from core import digest
from releases import CATALOG, CHANNELS, read_catalog, verify_signature
from workflow import VERSION

FIELDS = (
    ("distribution", "WSL distribution", "Ubuntu"),
    ("checkout", "Port checkout in WSL", ""),
    ("cache", "Builder cache in WSL (optional)", ""),
    ("devkitpro", "devkitPro root", "/opt/devkitpro"),
    ("rustc", "Verified Cafe Rust compiler", ""),
    ("shader-bin", "Cafe GLSL tools directory", ""),
    ("output", "Package output directory", ""),
    ("minecraft", "Private Minecraft installation", ""),
    ("client-jar", "Private Minecraft client JAR", ""),
    ("server-jar", "Private Minecraft server JAR", ""),
    ("output-resources", "Private resource output", ""),
)


class BuilderWindow:
    def __init__(self, root, catalog, payload, args):
        root.title(f"MCHorizonWiiU Builder {VERSION}")
        root.geometry("850x710")
        root.minsize(700, 590)
        self.root, self.args = root, args
        self.fingerprint = digest(payload)
        self.events = queue.Queue()
        self.process = None
        self.close_after_cancel = False
        self.settings = {}
        self.commands = {}
        self.offline = tk.BooleanVar()
        self.fixed = tk.BooleanVar(value=True)
        panel = ttk.Frame(root, padding=16)
        panel.pack(fill="both", expand=True)
        panel.columnconfigure(1, weight=1)
        ttk.Label(panel, text="MCHorizonWiiU Builder", font=("Segoe UI", 18, "bold")).grid(row=0, columnspan=2, sticky="w")
        channel_text = "Unsigned development catalog" if args.allow_unsigned else "Verified publisher catalog"
        ttk.Label(panel, text=channel_text).grid(row=1, columnspan=2, sticky="w", pady=(0, 8))
        self.options = [rule for rule in catalog["compatibility"] if rule["builder"] == VERSION
                        and all(catalog["releases"][kind][rule[kind]]["status"] != "revoked"
                                for kind in ("builder", "client", "patch", "toolchain"))]
        if not self.options:
            raise ValueError("no compatible releases are available for this builder")
        labels = [f"{rule['channel'].title()}  ·  Client {rule['client']}  ·  Patch {rule['patch']}"
                  f"  ·  Toolchain {rule['toolchain']}  ·  Horizon {rule['upstream'][:12]}"
                  for rule in self.options]
        ttk.Label(panel, text="Compatible release").grid(row=2, column=0, sticky="w")
        self.selection = ttk.Combobox(panel, state="readonly", values=labels)
        self.selection.current(0)
        self.selection.grid(row=2, column=1, sticky="ew", pady=2)
        for row, (key, label, default) in enumerate(FIELDS, 3):
            value = tk.StringVar(value=default)
            self.settings[key] = value
            ttk.Label(panel, text=label).grid(row=row, column=0, sticky="w", padx=(0, 12))
            ttk.Entry(panel, textvariable=value).grid(row=row, column=1, sticky="ew", pady=2)
        row = len(FIELDS) + 3
        ttk.Label(panel, text="Enter WSL paths, such as /home/me/project or /mnt/c/Builds.").grid(
            row=row, columnspan=2, sticky="w", pady=(2, 8))
        options = ttk.Frame(panel)
        options.grid(row=row + 1, columnspan=2, sticky="w")
        ttk.Checkbutton(options, variable=self.offline, text="Offline").pack(side="left")
        ttk.Checkbutton(options, variable=self.fixed, text="Fixed compiler path").pack(side="left", padx=12)
        actions = ttk.Frame(panel)
        actions.grid(row=row + 2, columnspan=2, sticky="ew", pady=8)
        for action, label in (("inventory-toolchain", "Check toolchain"), ("prepare", "Prepare / repair dependencies"),
                              ("prepare-resources", "Prepare resources"), ("build", "Build RPX / WUHB")):
            button = ttk.Button(actions, text=label, command=lambda action=action: self.start(action))
            button.pack(side="left", padx=(0, 6))
            self.commands[action] = button
        self.cancel = ttk.Button(actions, text="Cancel", state="disabled", command=self.stop)
        self.cancel.pack(side="left")
        self.progress = ttk.Progressbar(panel, mode="indeterminate")
        self.progress.grid(row=row + 3, columnspan=2, sticky="ew")
        self.status = tk.StringVar(value="Ready")
        ttk.Label(panel, textvariable=self.status).grid(row=row + 4, columnspan=2, sticky="w", pady=4)
        log_frame = ttk.Frame(panel)
        log_frame.grid(row=row + 5, columnspan=2, sticky="nsew")
        panel.rowconfigure(row + 5, weight=1)
        self.log = tk.Text(log_frame, height=8, wrap="word", state="disabled")
        scrollbar = ttk.Scrollbar(log_frame, command=self.log.yview)
        self.log.configure(yscrollcommand=scrollbar.set)
        scrollbar.pack(side="right", fill="y")
        self.log.pack(fill="both", expand=True)
        ttk.Button(panel, text="SD deployment", command=self.deployment).grid(row=row + 6, column=1, sticky="e", pady=(6, 0))
        self.root.protocol("WM_DELETE_WINDOW", self.close)
        self.root.after(75, self.poll)

    def deployment(self):
        messagebox.showinfo("SD deployment",
                            "Create wiiu/apps/HorizonWiiU on a FAT32 SD card. Copy HorizonWiiU.wuhb "
                            "for Aroma or HorizonWiiU.rpx for an RPX-capable launcher. Place your "
                            "private prepared resources in .local/horizon-client/26.3 inside that app "
                            "folder. Use a scratch Cemu SD for first launch, and back up any existing "
                            "worlds before testing a development build.",
                            parent=self.root)

    def required(self, key):
        value = self.settings[key].get().strip()
        if not value:
            raise ValueError(f"Enter the {key.replace('-', ' ')}.")
        return value

    def start(self, action):
        if self.process is not None:
            return
        rule = self.options[self.selection.current()]
        try:
            distribution = self.required("distribution")
            checkout = self.required("checkout").rstrip("/")
            if not checkout.startswith("/"):
                raise ValueError("Enter the absolute WSL checkout path.")
            wsl = ["wsl.exe", "--distribution", distribution, "--cd", checkout, "--exec"]
            command = wsl + ["python3", "-B", "wiiu/builder/wsl_job.py",
                             self.settings["devkitpro"].get().strip(),
                             self.settings["rustc"].get().strip(),
                             self.settings["shader-bin"].get().strip(),
                             "--catalog-sha256", self.fingerprint]
            for key in ("client", "patch", "toolchain", "channel"):
                command += [f"--{key}", rule[key]]
            if self.args.allow_unsigned:
                command.append("--allow-unsigned")
            else:
                for flag, path in (("--catalog", self.args.catalog), ("--signature", self.args.signature),
                                   ("--public-key", self.args.public_key)):
                    linux_path = subprocess.check_output(wsl + ["wslpath", "-a", str(path.resolve())],
                                                         text=True, encoding="utf-8").strip()
                    command += [flag, linux_path]
            if self.offline.get():
                command.append("--offline")
            if cache := self.settings["cache"].get().strip():
                command += ["--cache", cache]
            command.append(action)
            if action == "prepare":
                command += ["--inputs", "auto"]
            if action == "build":
                command += ["--inputs", "auto", "--output", self.required("output")]
                if self.fixed.get():
                    command.append("--fixed-path")
                if resources := self.settings["output-resources"].get().strip():
                    command += ["--resources", resources, "--resource-manifest", resources.rstrip("/") + ".inventory.json"]
            if action == "prepare-resources":
                for flag, key in (("--minecraft", "minecraft"), ("--client-jar", "client-jar"),
                                  ("--server-jar", "server-jar"), ("--output", "output-resources")):
                    command += [flag, self.required(key)]
            self.process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                            stderr=subprocess.STDOUT, text=True, encoding="utf-8", errors="replace",
                                            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        except (OSError, ValueError, subprocess.CalledProcessError) as error:
            messagebox.showerror("Builder setup", str(error), parent=self.root)
            return
        for button in self.commands.values():
            button.configure(state="disabled")
        self.cancel.configure(state="normal")
        self.progress.start()
        self.status.set(self.commands[action].cget("text") + "…")
        threading.Thread(target=self.read_process, args=(self.process,), daemon=True).start()

    def read_process(self, process):
        with process.stdout:
            for line in process.stdout:
                self.events.put(("log", line))
        self.events.put(("done", process.wait()))

    def stop(self):
        if self.process and self.process.stdin:
            self.status.set("Cancelling…")
            self.cancel.configure(state="disabled")
            try:
                self.process.stdin.close()
            except OSError:
                pass

    def close(self):
        if self.process is None:
            self.root.destroy()
        elif messagebox.askyesno("Cancel operation", "Cancel the running job and close after cleanup?", parent=self.root):
            self.close_after_cancel = True
            self.stop()

    def poll(self):
        for _ in range(100):
            try:
                kind, value = self.events.get_nowait()
            except queue.Empty:
                break
            if kind == "log":
                self.log.configure(state="normal")
                self.log.insert("end", value)
                self.log.see("end")
                self.log.configure(state="disabled")
            else:
                self.progress.stop()
                self.cancel.configure(state="disabled")
                self.process.stdin.close()
                self.process = None
                if self.close_after_cancel:
                    self.root.destroy()
                    return
                for button in self.commands.values():
                    button.configure(state="normal")
                self.status.set({0: "Completed", 130: "Cancelled"}.get(value, f"Failed ({value}). See the build log."))
        self.root.after(75, self.poll)


def main():
    import os

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--catalog", type=Path, default=CATALOG)
    parser.add_argument("--allow-unsigned", action="store_true")
    parser.add_argument("--signature", type=Path)
    parser.add_argument("--public-key", type=Path)
    args = parser.parse_args()
    root = tk.Tk()
    try:
        if args.allow_unsigned and not (args.signature or args.public_key):
            catalog, data = read_catalog(args.catalog)
            if catalog["signing"] != "unsigned-development":
                raise ValueError("this catalog requires a signature and trusted key")
        elif args.signature and args.public_key and not args.allow_unsigned:
            state = Path(os.environ.get("LOCALAPPDATA", Path.home())) / "MCHorizonWiiU" / "trust"
            catalog, data = verify_signature(args.catalog, args.signature, args.public_key,
                                             state / (digest(args.public_key.read_bytes()) + ".json"))
        else:
            raise ValueError("Use --allow-unsigned for the development checkout or provide --signature and --public-key.")
        BuilderWindow(root, catalog, data, args)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        messagebox.showerror("MCHorizonWiiU Builder", str(error), parent=root)
        root.destroy()
        return 1
    root.mainloop()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
