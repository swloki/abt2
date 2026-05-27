import json
import networkx as nx
from networkx.readwrite import json_graph
from pathlib import Path

data = json.loads(Path('graphify-out/graph.json').read_text(encoding="utf-8"))
G = json_graph.node_link_graph(data, edges='links')

# Find key_to_i64
for nid in G.nodes():
    if 'key_to_i64' in G.nodes[nid].get('label', '').lower():
        data_n = G.nodes[nid]
        print(f'NODE: {data_n.get("label", nid)}')
        print(f'  source: {data_n.get("source_file","unknown")}')
        print(f'  degree: {G.degree(nid)}')
        print()
        print('ALL CONNECTIONS:')
        
        # Group neighbors by community
        from collections import defaultdict
        by_comm = defaultdict(list)
        for neighbor in G.neighbors(nid):
            edge = G[nid][neighbor]
            nlabel = G.nodes[neighbor].get('label', neighbor)
            rel = edge.get('relation', '')
            conf = edge.get('confidence', '')
            src = G.nodes[neighbor].get('source_file', '')
            comm = G.nodes[neighbor].get('community', '?')
            by_comm[comm].append(f'{nlabel} --{rel}--> [{conf}] ({src})')
        
        for comm, lines in sorted(by_comm.items(), key=lambda x: -len(x[1])):
            print(f'\n  Community: {comm}')
            for line in lines:
                print(f'    {line}')
        break