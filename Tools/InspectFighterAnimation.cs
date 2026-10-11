using System;
using System.Linq;
using System.Text;
using Guildbound.Anim;
using Guildbound.Play;
using UnityEditor;
using UnityEngine;
using UnityEngine.InputSystem;

public static class InspectFighterAnimation
{
    public static string Execute()
    {
        if (!EditorApplication.isPlaying) throw new InvalidOperationException("Inspect during Play Mode.");
        var driver = UnityEngine.Object.FindFirstObjectByType<SparringDriver>();
        var b = new StringBuilder();
        foreach (var view in new[] { driver.PlayerView, driver.OpponentView })
        {
            var anim = view.GetComponentInChildren<FighterAnimation>();
            var rig = anim.GetComponent<Animator>();
            var state = view.Current;
            b.AppendLine(view.name + " id=" + view.Id + " rule=" + state.Kind + " seq=" + state.Sequence + " elapsed=" + state.Elapsed
                + " pose=" + anim.PresentedState + " source=" + anim.SourceSeconds + " root=" + anim.transform.localPosition
                + " hips=" + rig.GetBoneTransform(HumanBodyBones.Hips).localPosition
                + " Lhand=" + anim.transform.InverseTransformPoint(rig.GetBoneTransform(HumanBodyBones.LeftHand).position)
                + " Rhand=" + anim.transform.InverseTransformPoint(rig.GetBoneTransform(HumanBodyBones.RightHand).position));
        }
        b.AppendLine("Keys held: " + string.Join(", ", Keyboard.current.allKeys.Where(k => k.isPressed).Select(k => k.name)));
        b.AppendLine("Mouse held: " + Mouse.current.leftButton.isPressed + "/" + Mouse.current.rightButton.isPressed);
        return b.ToString();
    }
}
