{
  installShellFiles,
  lib,
  rustPlatform,
  stdenv,
  gitHash ? "unknown",
}:

rustPlatform.buildRustPackage (finalAttrs: {
  pname = "ldk-server";
  version = (builtins.fromTOML (builtins.readFile ../ldk-server/Cargo.toml)).package.version;

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.lock
      ../Cargo.toml
      ../LICENSE-APACHE
      ../LICENSE-MIT
      ../ldk-server
      ../ldk-server-cli
      ../ldk-server-client
      ../ldk-server-grpc
      ../ldk-server-mcp
    ];
  };

  cargoLock = {
    lockFile = ../Cargo.lock;
    outputHashes = {
      "bitcoin-payment-instructions-0.6.0" = "sha256-ZhnZopZgBIsoIKHxWmCfQDrpGpYLSedrC+Odj0Mbbo8=";
      "ldk-node-0.8.0+git" = "sha256-YgdxkOdS1bvG7NWGKgzXiLqw0ciTqHMJghk/bvQEG3A=";
      "lightning-0.3.0+git" = "sha256-o4NT1unDxzQ9TlPpkqsB1G7Qh9vV6ZvqPEKR+2smsvY=";
    };
  };

  GIT_HASH = gitHash;

  nativeBuildInputs = [ installShellFiles ];

  cargoBuildFlags = [
    "-p"
    "ldk-server"
    "-p"
    "ldk-server-cli"
  ];
  cargoTestFlags = finalAttrs.cargoBuildFlags;
  checkType = "debug";

  installPhase = ''
    runHook preInstall

    releaseDir="target/${stdenv.hostPlatform.rust.rustcTarget}/release"
    install -Dm755 "$releaseDir/ldk-server" "$out/bin/ldk-server"
    install -Dm755 "$releaseDir/ldk-server-cli" "$out/bin/ldk-server-cli"

    ${lib.optionalString (stdenv.buildPlatform.canExecute stdenv.hostPlatform) ''
      completionDir="$(mktemp -d)"
      "$releaseDir/ldk-server-cli" completions bash > "$completionDir/ldk-server-cli.bash"
      "$releaseDir/ldk-server-cli" completions fish > "$completionDir/ldk-server-cli.fish"
      "$releaseDir/ldk-server-cli" completions zsh > "$completionDir/_ldk-server-cli"

      installShellCompletion --cmd ldk-server-cli \
        --bash "$completionDir/ldk-server-cli.bash" \
        --fish "$completionDir/ldk-server-cli.fish" \
        --zsh "$completionDir/_ldk-server-cli"
    ''}

    runHook postInstall
  '';

  meta = {
    description = "Ready-to-run Lightning node daemon built with LDK Node";
    homepage = "https://github.com/lightningdevkit/ldk-server";
    license = with lib.licenses; [
      asl20
      mit
    ];
    mainProgram = "ldk-server";
  };
})
