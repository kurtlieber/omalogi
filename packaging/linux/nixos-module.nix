{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.omalogi;
in
{
  options.programs.omalogi = {
    enable = lib.mkEnableOption "Omalogi, a local-first Logitech device manager";

    package = lib.mkPackageOption pkgs "omalogi" { };

    launchAtLogin = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Whether to start the Omalogi agent with graphical sessions.";
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];
    services.udev.packages = [ cfg.package ];

    systemd.user.services.omalogi-agent = {
      description = "Omalogi background agent";
      wantedBy = lib.optionals cfg.launchAtLogin [ "graphical-session.target" ];
      after = [ "graphical-session.target" ];
      partOf = lib.optionals cfg.launchAtLogin [ "graphical-session.target" ];

      serviceConfig = {
        ExecStart = lib.getExe' cfg.package "omalogi-agent";
        Restart = "on-failure";
        RestartSec = 5;
      };
    };
  };
}
