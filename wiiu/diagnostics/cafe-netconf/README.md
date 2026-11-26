# Cafe configured DNS probe

This standalone Wii U app checks whether the console can expose the IPv4
resolvers assigned to its active network. It uses optional nsysnet exports
and logs return codes and counts. It reads no game files, credentials, or
network profile contents.

Build with WUT and devkitPPC from [Cafe Toolchain 1.1.0](../../../docs/gx2/cafe-toolchain-1.1.0.md).

~~~sh
bash wiiu/diagnostics/cafe-netconf/build.sh "$scratch"
~~~

| Artifact | SHA-256 |
| --- | --- |
| RPX | `ba9b45f3348bab148f9aada8af02a96ec3af5b11c65b7728bfbffe4b47dbfcc8` |
| WUHB | `7ecc665a97719b2ce688d2503bf3f7da50080fe4bb1cb80e2884d3320b1b8831` |

On Wii U, launch the WUHB from its own SD application directory with the
console already connected. Preserve serial records through the ProcUI exit
marker. Check the Wi-Fi and Ethernet results separately. A successful
result needs at least one usable resolver on the active interface.

In NSS-patched Cemu 2.6 forced interpreter, initialization and shutdown
of NetConf returned zero. Both interfaces reported zero nameservers and
the result was `unavailable`. The headless fixture did not finish the
menu handoff in Cemu within 30 seconds. The test runner stopped its
own process. The production resolver and direct host-and-port joins
are unchanged. [Online-mode qualification](../../../docs/gx2/online-mode.md)
