import importlib.util, pathlib, unittest

PATH=pathlib.Path(__file__).parents[1]/"deploy/routing/zpnet-routing-agent.py"
spec=importlib.util.spec_from_file_location("routing_agent",PATH); agent=importlib.util.module_from_spec(spec); spec.loader.exec_module(agent)
CFG={"client_subnet":"10.66.0.0/24","main_table":100,"media_table":200,"exits":[
 {"id":"main","name":"Main","public_ip":"88.214.54.68","peer_ip":"10.88.88.2","interface":"ru-extra","table":100,"role":"main","health_url":"http://10.88.88.2:18081/health"},
 {"id":"nl","name":"NL","public_ip":"87.58.210.58","peer_ip":"10.99.99.2","interface":"ru-youtube","table":200,"role":"youtube","health_url":"http://10.99.99.2:18081/health"}]}
class RoutingTests(unittest.TestCase):
 def test_normal(self):
  routes,_=agent.select_routes(CFG["exits"],{"main":{"healthy":True},"nl":{"healthy":True}}); self.assertEqual(routes,{"100":"main","200":"nl"})
 def test_failover(self):
  routes,_=agent.select_routes(CFG["exits"],{"main":{"healthy":False},"nl":{"healthy":True}}); self.assertEqual(routes,{"100":"nl","200":"nl"})
 def test_return_and_fail_closed(self):
  routes,_=agent.select_routes(CFG["exits"],{"main":{"healthy":True},"nl":{"healthy":False}}); self.assertEqual(routes,{"100":"main","200":"main"})
  routes,_=agent.select_routes(CFG["exits"],{}); self.assertEqual(routes,{"100":None,"200":None})
 def test_health_must_use_tunnel_peer(self):
  bad=dict(CFG); bad["exits"]=[dict(CFG["exits"][0],health_url="http://127.0.0.1:1/health"),CFG["exits"][1]]
  with self.assertRaises(ValueError): agent.validate(bad)
 def test_ipset_validation(self):
  self.assertEqual(agent.validated_cidrs("8.8.8.0/24\n8.8.8.1/24\n"),["8.8.8.0/24"])
  with self.assertRaises(ValueError): agent.validated_cidrs("10.0.0.0/8")
if __name__=="__main__": unittest.main()
