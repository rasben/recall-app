<script>
</script>
<style>
    .travolta-walk {
        animation: walk 6s linear infinite;
    }

    .travolta-bounce {
        animation: bounce 0.5s ease-in-out infinite;
    }

    @keyframes walk {
        from {
            transform: translateX(150px);
        }
        to {
            transform: translateX(calc(-100vw - 150px));
        }
    }

    @keyframes bounce {
        0%, 100% {
            transform: translateY(0);
        }
        50% {
            transform: translateY(-10px);
        }
    }
</style>


<div class="fixed -bottom-3 h-50 w-full">
    <div class="travolta-walk absolute right-0 h-50">
        <img src="./travolta.gif" class="travolta-bounce h-50 w-auto" alt="" />
    </div>
</div>
