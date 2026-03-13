# Android Build on Windows (ech0)

## Recomendação: Use WSL2 para Android Build

Build Android nativo em Windows tem complexidades relacionadas a OpenSSL e Perl/Unix tools. A forma mais prática é usar WSL2:

### WSL2 Setup (recomendado)

1. **Instale WSL2** (se não tiver):
   ```powershell
   wsl --install Ubuntu-24.04
   ```

2. **Dentro do WSL2 (Ubuntu)**:
   ```bash
   # Instalar Node.js
   curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
   sudo apt install nodejs

   # Instalar Rust e rustup
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source $HOME/.cargo/env

   # Adicionar targets Android
   rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android

   # Instalar JDK 17
   sudo apt install openjdk-17-jdk

   # Instalar Android SDK/NDK via Android Studio command-line tools
   # Ou reutilizar a instalação Windows via symlink
   ```

3. **Clonar/acessar projeto**:
   ```bash
   cd /mnt/c/Users/rafae/Desktop/projetos/ech0
   ```

4. **Rodar build**:
   ```bash
   chmod +x scripts/build.sh
   ./scripts/build.sh android
   ```

---

## Windows Native (Advanced - não recomendado)

Se quiser tentar Android nativo em Windows:

### Pré-requisitos Extras

- **Perl Unix-like**: Use MSYS2 ou Git Bash, não Strawberry Perl
  ```powershell
  # MSYS2 method (recomendado se quiser Windows native)
  choco install msys2
  # Em MSYS2:
  pacman -Syu
  pacman -S mingw-w64-x86_64-openssl mingw-w64-x86_64-perl
  ```

- Ou configure `OPENSSL_DIR` para apontar a uma build OpenSSL pré-compilada para Android

---

## Troubleshooting

**"perl not found"**: Use WSL2 ou MSYS2 com Perl Unix-like

**"OpenSSL configure failed"**: Windows Perl não funciona com OpenSSL cross-compile para Android

**Recomendação final**: **Use macOS, Linux ou WSL2 para Android build**
