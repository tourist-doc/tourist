import json
from subprocess import Popen, PIPE


class Tourist:
    def __init__(self):
        self.proc = Popen(["target/debug/tourist", "serve"],
                          stdout=PIPE, stdin=PIPE)

    def __rpc(self, method, args):
        msg = {
            "jsonrpc": "2.0",
            "method": method,
            "params": args,
            "id": 1
        }
        self.proc.stdin.write((json.dumps(msg) + "\n").encode("utf-8"))
        self.proc.stdin.flush()
        return json.loads(self.proc.stdout.readline().decode("utf-8"))

    def list_tours(self):
        return self.__rpc("list_tours", [])

    def create_tour(self, title):
        return self.__rpc("create_tour", [title])

    def open_tour(self, path, edit):
        return self.__rpc("open_tour", [path, edit])

    def set_tour_edit(self, tour_id, edit):
        return self.__rpc("set_tour_edit", [tour_id, edit])

    def view_tour(self, tour_id):
        return self.__rpc("view_tour", [tour_id])

    def edit_tour_metadata(self, tour_id, delta):
        return self.__rpc("edit_tour_metadata", [tour_id, delta])

    def forget_tour(self, tour_id):
        return self.__rpc("forget_tour", [tour_id])

    def create_stop(self, tour_id, title, path, line):
        return self.__rpc("create_stop", [tour_id, title, path, line])

    def view_stop(self, tour_id, stop_id):
        return self.__rpc("view_stop", [tour_id, stop_id])

    def edit_stop_metadata(self, tour_id, stop_id, delta):
        return self.__rpc("edit_stop_metadata", [tour_id, stop_id, delta])

    def link_stop(self, tour_id, stop_id, other_tour_id, other_stop_id):
        return self.__rpc(
            "link_stop", [tour_id, stop_id, other_tour_id, other_stop_id]
        )

    def locate_stop(self, tour_id, stop_id, naive):
        return self.__rpc("locate_stop", [tour_id, stop_id])

    def remove_stop(self, tour_id, stop_id):
        return self.__rpc("remove_stop", [tour_id, stop_id])

    def refresh_tour(self, tour_id, commit):
        return self.__rpi("refresh_tour", [tour_id, commit])

    def save_tour(self, tour_id, path):
        return self.__rpc("save_tour", [tour_id, path])

    def save_all(self):
        return self.__rpc("save_all", [])

    def delete_tour(self, tour_id):
        return self.__rpc("delete_tour", [tour_id])

    def index_repository(self, repo_name, path):
        return self.__rpc("index_repository", [repo_name, path])


if __name__ == "__main__":
    tourist = Tourist()
    tourist.index_repository(
        "tourist-core", "/home/harrison/Projects/tourist-core"
    )

    tour_id = tourist.create_tour("My tour")["result"]
    stop_id = tourist.create_stop(
        tour_id,
        "A stop",
        "/home/harrison/Projects/tourist-core/README.md",
        5
    )["result"]
