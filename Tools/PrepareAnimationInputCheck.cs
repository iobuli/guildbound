using System;
using System.Linq;
using Guildbound.Play;
using UnityEditor;
using UnityEngine;
using UnityEngine.InputSystem;

public static class PrepareAnimationInputCheck
{
    public static string Execute()
    {
        if (!EditorApplication.isPlaying) throw new InvalidOperationException("Run in Sparring Play Mode.");
        foreach (var device in InputSystem.devices)
            if (device is Keyboard || device is Mouse) InputSystem.ResetDevice(device);
        var driver = UnityEngine.Object.FindFirstObjectByType<SparringDriver>();
        driver.pattern = OpponentBrain.Pattern.Defensive;
        driver.ResetBout();
        return "Defensive partner, fresh bout, keyboard/mouse transient states reset. Current keyboard=" + Keyboard.current.name
            + ", mouse=" + Mouse.current.name + "; devices=" + string.Join(",", InputSystem.devices.Select(d => d.name + ":" + d.deviceId));
    }
}
