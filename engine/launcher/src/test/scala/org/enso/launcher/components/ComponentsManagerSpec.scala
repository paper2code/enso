package org.enso.launcher.components

import nl.gn0s1s.bump.SemVer
import org.enso.launcher.config.GlobalConfigurationManager
import org.enso.loggingservice.{LogLevel, TestLogger}

class ComponentsManagerSpec extends ComponentsManagerTest {

  "ComponentsManager" should {
    "find the latest engine version in semver ordering " +
    "(skipping broken releases)" in {
      val componentsManager = makeComponentsManager()
      componentsManager.fetchLatestEngineVersion() shouldEqual SemVer(0, 0, 1)
    }

    "install the engine and a matching runtime for it" in {
      val (distributionManager, componentsManager, _) = makeManagers()

      val version = SemVer(0, 0, 1)
      val engine  = componentsManager.findOrInstallEngine(SemVer(0, 0, 1))

      engine.version shouldEqual version
      assert(
        engine.path.startsWith(distributionManager.paths.engines),
        "Engine should be installed in the engines directory."
      )

      val runtime = componentsManager.findRuntime(engine)
      runtime.value.version shouldEqual RuntimeVersion(SemVer(2, 0, 0), "11")
      assert(
        runtime.value.path.startsWith(distributionManager.paths.runtimes),
        "Engine should be installed in the engines directory."
      )
    }

    "list installed engines and runtimes" in {
      val componentsManager = makeComponentsManager()
      val engineVersions =
        Set(SemVer(0, 0, 0), SemVer(0, 0, 1), SemVer(0, 0, 1, Some("pre")))
      val runtimeVersions =
        Set(
          RuntimeVersion(SemVer(1, 0, 0), "11"),
          RuntimeVersion(SemVer(2, 0, 0), "11")
        )
      engineVersions.map(
        componentsManager.findOrInstallEngine(_, complain = false)
      )

      componentsManager
        .listInstalledEngines()
        .map(_.version)
        .toSet shouldEqual engineVersions
      componentsManager
        .listInstalledRuntimes()
        .map(_.version)
        .toSet shouldEqual runtimeVersions

      val runtime2 =
        componentsManager
          .findRuntime(RuntimeVersion(SemVer(2, 0, 0), "11"))
          .value
      componentsManager.findEnginesUsingRuntime(runtime2) should have length 2
    }

    "preserve the broken mark when installing a broken release" in {
      val componentsManager = makeComponentsManager()
      val brokenVersion     = SemVer(0, 999, 0, Some("marked-broken"))
      componentsManager.findOrInstallEngine(
        brokenVersion,
        complain = false
      )

      assert(
        componentsManager.findEngine(brokenVersion).value.isMarkedBroken,
        "The broken release should still be marked as broken after being " +
        "installed and loaded."
      )
    }

    "skip broken releases when finding latest installed version" in {
      val (distributionManager, componentsManager, _) = makeManagers()
      val configurationManager =
        new GlobalConfigurationManager(componentsManager, distributionManager)

      val validVersion          = SemVer(0, 0, 1)
      val newerButBrokenVersion = SemVer(0, 999, 0, Some("marked-broken"))
      componentsManager.findOrInstallEngine(validVersion)
      componentsManager.findOrInstallEngine(newerButBrokenVersion)

      configurationManager.defaultVersion shouldEqual validVersion
    }

    "issue a warning when a broken release is requested" in {
      val componentsManager = makeComponentsManager()

      val brokenVersion = SemVer(0, 999, 0, Some("marked-broken"))
      val logs = TestLogger.gatherLogs {
        componentsManager.findOrInstallEngine(brokenVersion, complain = false)
      }
      val warnings = logs.filter(_.logLevel == LogLevel.Warning)
      warnings should have size 1
      val expectedWarning = warnings.head.message
      expectedWarning should include("is marked as broken")
      expectedWarning should include("consider changing")
      componentsManager.findEngine(brokenVersion).value
    }

    "uninstall the runtime iff it is not used by any engines" in {
      val componentsManager = makeComponentsManager()
      val engineVersions =
        Seq(SemVer(0, 0, 0), SemVer(0, 0, 1), SemVer(0, 0, 1, Some("pre")))
      engineVersions.map(
        componentsManager.findOrInstallEngine(_, complain = false)
      )

      componentsManager.listInstalledEngines() should have length 3
      componentsManager.listInstalledRuntimes() should have length 2

      // remove the engine that shares the runtime with another one
      val version1 = SemVer(0, 0, 1, Some("pre"))
      componentsManager.uninstallEngine(version1)
      val engines1 = componentsManager.listInstalledEngines()
      engines1 should have length 2
      engines1.map(_.version) should not contain version1
      componentsManager.listInstalledRuntimes() should have length 2

      // remove the second engine that shared the runtime
      val version2 = SemVer(0, 0, 1)
      componentsManager.uninstallEngine(version2)
      val engines2 = componentsManager.listInstalledEngines()
      engines2 should have length 1
      engines2.map(_.version) should not contain version2
      val runtimes2 = componentsManager.listInstalledRuntimes()
      runtimes2 should have length 1
      runtimes2.map(_.version).head shouldEqual RuntimeVersion(
        SemVer(1, 0, 0),
        "11"
      )

      // remove the last engine
      componentsManager.uninstallEngine(SemVer(0, 0, 0))
      componentsManager.listInstalledEngines() should have length 0
      componentsManager.listInstalledRuntimes() should have length 0
    }
  }
}
