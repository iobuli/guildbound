using System.Collections.Generic;
using Guildbound.Combat;
using Guildbound.Contracts;
using UnityEngine;
using UnityEngine.InputSystem;

namespace Guildbound.Play
{
    // Runs the M0.5 sparring fight: owned combat core at a fixed step, player input (GDD keys), the sparring partner,
    // presentation handles and HUD/alert events. Enter resets the bout; P switches the partner pattern.
    public class SparringDriver : MonoBehaviour
    {
        public Transform spawnA, spawnB;
        public Camera viewCamera;
        public bool playerAttuned = true;
        public int playerPrepItems = 1;
        public OpponentBrain.Pattern pattern = OpponentBrain.Pattern.Pressure;

        public CombatWorld World { get; private set; }
        public FighterSim Player => World.Fighters[0];
        public FighterSim Opponent => World.Fighters[1];
        public FighterView PlayerView { get; private set; }
        public FighterView OpponentView { get; private set; }
        public readonly Queue<HudAlert> Alerts = new Queue<HudAlert>();
        public double LastCastInterrupt = -10;

        FixedStepRunner runner;
        OpponentBrain brain;
        readonly List<Intent> intents = new List<Intent> { default, default };
        readonly List<GameObject> projectileViews = new List<GameObject>();
        bool locked = true;

        void Start() => ResetBout();

        public void ResetBout()
        {
            World = new CombatWorld();
            var a = spawnA ? spawnA.position : new Vector3(-1.5f, 0, 0);
            var b = spawnB ? spawnB.position : new Vector3(1.5f, 0, 0);
            var p = World.Add(new Vector3(a.x, 0, a.z), (b - a).normalized, 100f);
            var o = World.Add(new Vector3(b.x, 0, b.z), (a - b).normalized, 60f);
            p.Attuned = playerAttuned;
            p.PrepItems = playerPrepItems;
            p.LockTarget = locked ? o.Id : -1;
            runner = new FixedStepRunner(World);
            brain = new OpponentBrain { pattern = pattern };
            PlayerView = View(PlayerView, "Player", p, new Color(0.35f, 0.6f, 1f));
            OpponentView = View(OpponentView, "Partner", o, new Color(0.85f, 0.35f, 0.3f));
            PlayerView.lockTarget = OpponentView.transform;
            OpponentView.lockTarget = PlayerView.transform;
        }

        FighterView View(FighterView existing, string name, FighterSim sim, Color tint)
        {
            var v = existing ? existing : new GameObject(name).AddComponent<FighterView>();
            v.Sim = sim;
            v.tint = tint;
            return v;
        }

        void Update()
        {
            var kb = Keyboard.current;
            if (kb != null && kb.enterKey.wasPressedThisFrame) ResetBout();
            if (kb != null && kb.pKey.wasPressedThisFrame)
            {
                pattern = pattern == OpponentBrain.Pattern.Pressure ? OpponentBrain.Pattern.Defensive : OpponentBrain.Pattern.Pressure;
                brain.pattern = pattern;
                Alerts.Enqueue(new HudAlert { Text = "Partner: " + pattern, Level = AlertLevel.Info });
            }
            var mouse = Mouse.current;
            if (mouse != null && mouse.middleButton.wasPressedThisFrame)
            {
                locked = !locked;
                Player.LockTarget = locked ? Opponent.Id : -1;
            }

            intents[0] = ReadPlayer(kb, mouse);
            intents[1] = brain.Think(World, Opponent, Player);
            runner.Frame(Time.deltaTime, intents);
            Dispatch();
            SyncProjectiles();
        }

        Intent ReadPlayer(Keyboard kb, Mouse mouse)
        {
            var i = new Intent();
            if (kb == null) return i;
            var m = new Vector2((kb.dKey.isPressed ? 1 : 0) - (kb.aKey.isPressed ? 1 : 0), (kb.wKey.isPressed ? 1 : 0) - (kb.sKey.isPressed ? 1 : 0));
            if (viewCamera && m.sqrMagnitude > 0)
            {
                var f = viewCamera.transform.forward; f.y = 0; f.Normalize();
                var r = viewCamera.transform.right; r.y = 0; r.Normalize();
                var w = f * m.y + r * m.x;
                m = new Vector2(w.x, w.z);
            }
            i.Move = Vector2.ClampMagnitude(m, 1f);
            i.Run = kb.leftShiftKey.isPressed;
            i.Light = mouse != null && mouse.leftButton.wasPressedThisFrame;
            i.GuardHeld = mouse != null && mouse.rightButton.isPressed;
            i.Heavy = kb.fKey.wasPressedThisFrame;
            i.Step = kb.leftCtrlKey.wasPressedThisFrame;
            i.Throw = kb.rKey.wasPressedThisFrame;
            i.Cast = kb.qKey.wasPressedThisFrame;
            i.CastHeld = kb.qKey.isPressed;
            i.Recover = kb.vKey.wasPressedThisFrame;
            return i;
        }

        void Dispatch()
        {
            foreach (var (fighter, state) in World.Started)
            {
                var v = fighter == 0 ? PlayerView : OpponentView;
                v.RaiseStarted(state);
            }
            foreach (var c in World.Contacts)
            {
                if (c.AttackerId >= 0) (c.AttackerId == 0 ? PlayerView : OpponentView).RaiseContact(c);
                if (c.DefenderId >= 0) (c.DefenderId == 0 ? PlayerView : OpponentView).RaiseContact(c);
                if (c.DefenderId == 0 && c.Outcome == Outcome.GuardBroken) Alerts.Enqueue(new HudAlert { Text = "Guard broken", Level = AlertLevel.Critical });
                if (c.DefenderId == 0 && c.Outcome == Outcome.Hit && (Player.LastKind == ActionKind.CastHold || Player.LastKind == ActionKind.CastPlain))
                {
                    LastCastInterrupt = World.Time;
                    Alerts.Enqueue(new HudAlert { Text = "Cast interrupted", Level = AlertLevel.Warning });
                }
                if (c.AttackerId == 0 && c.Action == ActionKind.Throw && c.Outcome == Outcome.Miss) Alerts.Enqueue(new HudAlert { Text = "Throw failed: no space or no target", Level = AlertLevel.Info });
            }
            if (World.Contacts.Count > 0)
            {
                if (Opponent.Defeated) Alerts.Enqueue(new HudAlert { Text = "Partner down - Enter to reset", Level = AlertLevel.Info });
                if (Player.Defeated) Alerts.Enqueue(new HudAlert { Text = "Defeated - Enter to reset", Level = AlertLevel.Critical });
            }
            World.ClearEvents();
        }

        void SyncProjectiles()
        {
            while (projectileViews.Count < World.Projectiles.Count)
            {
                var go = GameObject.CreatePrimitive(PrimitiveType.Sphere);
                Destroy(go.GetComponent<Collider>());
                go.transform.localScale = Vector3.one * 0.5f;
                go.GetComponent<Renderer>().material.color = new Color(1f, 0.55f, 0.15f);
                projectileViews.Add(go);
            }
            for (int i = 0; i < projectileViews.Count; i++)
            {
                bool on = i < World.Projectiles.Count;
                projectileViews[i].SetActive(on);
                if (!on) continue;
                var pr = World.Projectiles[i];
                projectileViews[i].transform.position = pr.Position + Vector3.up * 1.3f;
                projectileViews[i].transform.localScale = Vector3.one * (pr.Charged ? 0.8f : 0.5f);
            }
        }
    }
}
