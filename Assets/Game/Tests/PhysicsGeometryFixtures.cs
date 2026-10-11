using System.Collections.Generic;
using Guildbound.Combat;
using NUnit.Framework;
using UnityEditor;
using UnityEngine;
using UnityEngine.SceneManagement;

namespace Guildbound.Tests
{
    // PhysicsGeometry against real colliders (M1 W1). The colliders are hidden, never saved and built 1 km from the
    // origin, so no open scene content is queried and the open scene is left untouched.
    public class PhysicsGeometryFixtures
    {
        static readonly Vector3 O = new Vector3(1000f, 0f, 1000f);
        const float R = 0.3f;
        readonly List<GameObject> made = new List<GameObject>();
        PhysicsGeometry geo;
        bool dirtyBefore;

        [SetUp]
        public void SetUp()
        {
            dirtyBefore = SceneManager.GetActiveScene().isDirty;
            Box(new Vector3(0f, -0.1f, 0f), new Vector3(20f, 0.2f, 20f));   // floor: must never block
            Box(new Vector3(0f, 1f, 2f), new Vector3(6f, 2f, 0.2f));        // wall across +Z, face at z = 1.9
            Box(new Vector3(1.5f, 1f, 0f), new Vector3(0.2f, 2f, 6f));      // wall across +X, face at x = 1.4
            Physics.SyncTransforms();
            geo = new PhysicsGeometry(Physics.DefaultRaycastLayers);
        }

        [TearDown]
        public void TearDown()
        {
            foreach (var go in made) Object.DestroyImmediate(go);
            made.Clear();
            Physics.SyncTransforms();
            Assert.AreEqual(dirtyBefore, SceneManager.GetActiveScene().isDirty, "the fixture must not dirty the open scene");
        }

        void Box(Vector3 centre, Vector3 size)
        {
            var go = EditorUtility.CreateGameObjectWithHideFlags("PhysicsGeometryFixture", HideFlags.HideAndDontSave, typeof(BoxCollider));
            go.transform.position = O + centre;
            go.GetComponent<BoxCollider>().size = size;
            made.Add(go);
        }

        static Vector3 P(float x, float z) => O + new Vector3(x, 0f, z);

        [Test]
        public void Constrain_StopsAtAWall_OneRadiusShort()
        {
            var end = geo.Constrain(P(0f, 0f), P(0f, 3f), R);
            Assert.AreEqual(1.9f - R, end.z - O.z, 0.02f);
            Assert.AreEqual(0f, end.y, "positions stay on the ground plane");
        }

        [Test]
        public void Constrain_SlidesAlongAWall()
        {
            var end = geo.Constrain(P(-2f, 0f), P(-1f, 3f), R);
            Assert.AreEqual(1.9f - R, end.z - O.z, 0.02f, "into-wall motion removed");
            Assert.AreEqual(-1f, end.x - O.x, 0.02f, "along-wall motion kept");
        }

        [Test]
        public void Constrain_CornerStopsBothAxes()
        {
            var end = geo.Constrain(P(0f, 0f), P(3f, 3f), R);
            Assert.AreEqual(1.4f - R, end.x - O.x, 0.02f);
            Assert.AreEqual(1.9f - R, end.z - O.z, 0.02f);
        }

        [Test]
        public void Constrain_FreeMove_IsExact_AndTheFloorNeverBlocks()
        {
            Assert.AreEqual(P(-1f, 1f), geo.Constrain(P(-2f, -2f), P(-1f, 1f), R));
        }

        [Test]
        public void Blocked_ReportsTheFractionToTheWall()
        {
            Assert.IsTrue(geo.Blocked(P(0f, 0f), P(0f, 4f), 0f, out var t));
            Assert.AreEqual(1.9f / 4f, t, 0.005f);
            Assert.IsFalse(geo.Blocked(P(-2f, 0f), P(-2f, 1.5f), 0f, out _));
        }

        [Test]
        public void Fits_IsFalseAgainstAWall_TrueInTheOpen()
        {
            Assert.IsFalse(geo.Fits(P(0f, 1.8f), R));
            Assert.IsTrue(geo.Fits(P(0f, 1f), R));
        }

        [Test]
        public void SameQueries_SameAnswers()
        {
            var a = geo.Constrain(P(0.3f, -0.7f), P(2.7f, 2.2f), R);
            var b = geo.Constrain(P(0.3f, -0.7f), P(2.7f, 2.2f), R);
            Assert.AreEqual(a, b, "the motor's frame-rate identity depends on repeatable world queries");
        }
    }
}
