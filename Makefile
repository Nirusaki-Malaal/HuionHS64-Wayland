PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin
UDEVDIR ?= /etc/udev/rules.d
SYSTEMDUSERDIR ?= $(PREFIX)/lib/systemd/user
APPDIR ?= $(PREFIX)/share/applications
MODULESDIR ?= /etc/modules-load.d

.PHONY: all build test test-kernel test-session test-lifecycle install uninstall clean

all: build

build:
	cargo build --release

test:
	cargo test --workspace

# Requires access to uinput and generated evdev nodes.
test-kernel:
	cargo test -p huion-daemon -- --include-ignored

# Requires the running installed user service.
test-session:
	python3 tests/session_e2e.py

# Briefly stops/restarts the installed user service.
test-lifecycle:
	python3 tests/lifecycle_e2e.py

install: build
	install -d $(DESTDIR)$(BINDIR)
	install -m 755 target/release/huion-daemon $(DESTDIR)$(BINDIR)/huion-daemon
	install -m 755 target/release/huion-gui $(DESTDIR)$(BINDIR)/huion-gui
	install -d $(DESTDIR)$(UDEVDIR)
	install -m 644 data/70-huion-wayland.rules $(DESTDIR)$(UDEVDIR)/70-huion-wayland.rules
	install -d $(DESTDIR)$(SYSTEMDUSERDIR)
	install -m 644 data/huion-wayland.service $(DESTDIR)$(SYSTEMDUSERDIR)/huion-wayland.service
	install -d $(DESTDIR)$(APPDIR)
	install -m 644 data/io.github.nirusaki.HuionWayland.desktop $(DESTDIR)$(APPDIR)/io.github.nirusaki.HuionWayland.desktop
	install -d $(DESTDIR)$(MODULESDIR)
	install -m 644 data/kalam-uinput.conf $(DESTDIR)$(MODULESDIR)/kalam-uinput.conf
	@echo "Installation complete. Reload udev rules using: sudo udevadm control --reload && sudo udevadm trigger"

uninstall:
	rm -f $(DESTDIR)$(BINDIR)/huion-daemon
	rm -f $(DESTDIR)$(BINDIR)/huion-gui
	rm -f $(DESTDIR)$(UDEVDIR)/70-huion-wayland.rules
	rm -f $(DESTDIR)$(SYSTEMDUSERDIR)/huion-wayland.service
	rm -f $(DESTDIR)$(APPDIR)/io.github.nirusaki.HuionWayland.desktop
	rm -f $(DESTDIR)$(MODULESDIR)/kalam-uinput.conf

JNI_LIBS_DIR := android/app/src/main/jniLibs

android-libs:
	@mkdir -p $(JNI_LIBS_DIR)/arm64-v8a $(JNI_LIBS_DIR)/armeabi-v7a $(JNI_LIBS_DIR)/x86_64
	cargo ndk -t arm64-v8a -o $(JNI_LIBS_DIR) build --release -p huion-jni
	cargo ndk -t armeabi-v7a -o $(JNI_LIBS_DIR) build --release -p huion-jni
	cargo ndk -t x86_64 -o $(JNI_LIBS_DIR) build --release -p huion-jni

android-apk: android-libs
	cd android && ./gradlew assembleDebug

clean:
	cargo clean
